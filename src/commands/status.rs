use std::cell::RefMut;
use std::collections::{BTreeMap, HashMap};
use std::io::Write;

use colored::Colorize;
use once_cell::sync::Lazy;

use crate::commands::{Command, CommandContext};
use crate::database::Database;
use crate::errors::Result;
use crate::refs::HEAD;
use crate::repository::pending_commit::PendingCommitType;
use crate::repository::status::Status as RepositoryStatus;
use crate::repository::ChangeType;

pub struct Status<'a> {
    ctx: CommandContext<'a>,
    status: RepositoryStatus,
    /// `jit status --porcelain`
    porcelain: bool,
}

static SHORT_STATUS: Lazy<HashMap<ChangeType, &'static str>> = Lazy::new(|| {
    HashMap::from([
        (ChangeType::Added, "A"),
        (ChangeType::Deleted, "D"),
        (ChangeType::Modified, "M"),
    ])
});
static LONG_STATUS: Lazy<HashMap<ChangeType, &'static str>> = Lazy::new(|| {
    HashMap::from([
        (ChangeType::Added, "new file:"),
        (ChangeType::Deleted, "deleted:"),
        (ChangeType::Modified, "modified:"),
    ])
});
static CONFLICT_SHORT_STATUS: Lazy<HashMap<Vec<u16>, &'static str>> = Lazy::new(|| {
    HashMap::from([
        (vec![1, 2, 3], "UU"),
        (vec![1, 2], "UD"),
        (vec![1, 3], "DU"),
        (vec![2, 3], "AA"),
        (vec![2], "AU"),
        (vec![3], "UA"),
    ])
});
static CONFLICT_LONG_STATUS: Lazy<HashMap<Vec<u16>, &'static str>> = Lazy::new(|| {
    HashMap::from([
        (vec![1, 2, 3], "both modified:"),
        (vec![1, 2], "deleted by them:"),
        (vec![1, 3], "deleted by us:"),
        (vec![2, 3], "both added:"),
        (vec![2], "added by us:"),
        (vec![3], "added by them:"),
    ])
});

static LABEL_WIDTH: usize = 12;
static CONFLICT_LABEL_WIDTH: usize = 17;

impl<'a> Status<'a> {
    pub fn new(mut ctx: CommandContext<'a>) -> Self {
        let porcelain = match ctx.opt.cmd {
            Command::Status { porcelain } => porcelain,
            _ => unreachable!(),
        };

        let status = ctx.repo.status(None);

        Self {
            ctx,
            status,
            porcelain,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        self.ctx.repo.index.load_for_update()?;
        self.status.initialize()?;
        self.ctx.repo.index.write_updates()?;

        self.print_results()?;

        Ok(())
    }

    fn print_results(&self) -> Result<()> {
        if self.porcelain {
            self.print_porcelain_format()?;
        } else {
            self.print_long_format()?;
        }

        Ok(())
    }

    fn print_porcelain_format(&self) -> Result<()> {
        let mut stdout = self.ctx.stdout.borrow_mut();

        for path in &self.status.changed {
            let status = self.status_for(path);
            writeln!(stdout, "{} {}", status, path)?;
        }
        for path in &self.status.untracked_files {
            writeln!(stdout, "?? {}", path)?;
        }

        Ok(())
    }

    fn print_long_format(&self) -> Result<()> {
        self.print_branch_status()?;
        self.print_pending_commit_status()?;

        self.print_changeset(
            "Changes to be committed",
            &self.status.index_changes,
            "green",
        )?;
        self.print_unmerged_paths()?;
        self.print_changeset(
            "Changes not staged for commit",
            &self.status.workspace_changes,
            "red",
        )?;
        self.print_untracked_files()?;

        self.print_commit_status()?;

        Ok(())
    }

    fn print_branch_status(&self) -> Result<()> {
        let current = self.ctx.repo.refs.current_ref(HEAD)?;

        let mut stdout = self.ctx.stdout.borrow_mut();
        if current.is_head() {
            writeln!(
                stdout,
                "{}",
                String::from("Not currently on any branch.").red()
            )?;
        } else {
            writeln!(
                stdout,
                "On branch {}",
                self.ctx.repo.refs.short_name(&current)
            )?;
        }

        Ok(())
    }

    fn print_pending_commit_status(&self) -> Result<()> {
        match self.ctx.repo.pending_commit().merge_type() {
            Some(PendingCommitType::Merge) => {
                let mut stdout = self.ctx.stdout.borrow_mut();

                if self.status.conflicts.is_empty() {
                    writeln!(stdout, "All conflicts fixed but you are still merging.")?;
                    self.hint(&mut stdout, "use 'jit commit' to conclude merge")?;
                } else {
                    writeln!(stdout, "You have unmerged paths.")?;
                    self.hint(&mut stdout, "fix conflicts and run 'jit commit'")?;
                    self.hint(&mut stdout, "use 'jit merge --abort' to abort the merge")?;
                }
                writeln!(stdout)?;
            }
            Some(PendingCommitType::CherryPick) => {
                self.print_pending_type(PendingCommitType::CherryPick)?
            }
            Some(PendingCommitType::Revert) => {
                self.print_pending_type(PendingCommitType::Revert)?
            }
            None => (),
        }

        Ok(())
    }

    fn print_pending_type(&self, merge_type: PendingCommitType) -> Result<()> {
        let oid = self.ctx.repo.pending_commit().merge_oid(merge_type)?;
        let short = Database::short_oid(&oid);
        let op = match merge_type {
            PendingCommitType::CherryPick => "cherry-pick",
            PendingCommitType::Revert => "revert",
            _ => unreachable!(),
        };

        let mut stdout = self.ctx.stdout.borrow_mut();

        writeln!(stdout, "You are currently {}ing commit {}.", op, short)?;

        if self.status.conflicts.is_empty() {
            self.hint(
                &mut stdout,
                &format!("all conflicts fixed: run 'jit {} --continue'", op),
            )?;
        } else {
            self.hint(
                &mut stdout,
                &format!("fix conflicts and run 'jit {} --continue'", op),
            )?;
        }
        self.hint(
            &mut stdout,
            &format!("use 'jit {} --abort' to cancel the {} operation", op, op),
        )?;
        writeln!(stdout)?;

        Ok(())
    }

    fn hint(&self, stdout: &mut RefMut<Box<dyn Write>>, message: &str) -> Result<()> {
        writeln!(stdout, "  ({})", message)?;

        Ok(())
    }

    fn print_changeset(
        &self,
        message: &str,
        changeset: &BTreeMap<String, ChangeType>,
        style: &str,
    ) -> Result<()> {
        if changeset.is_empty() {
            return Ok(());
        }

        let mut stdout = self.ctx.stdout.borrow_mut();

        writeln!(stdout, "{}:", message)?;
        writeln!(stdout)?;
        for (path, change_type) in changeset {
            let status = format!("{:width$}", LONG_STATUS[change_type], width = LABEL_WIDTH);
            writeln!(stdout, "{}", format!("\t{}{}", status, path).color(style))?;
        }
        writeln!(stdout)?;

        Ok(())
    }

    fn print_unmerged_paths(&self) -> Result<()> {
        if self.status.conflicts.is_empty() {
            return Ok(());
        }

        let mut stdout = self.ctx.stdout.borrow_mut();

        writeln!(stdout, "Unmerged paths:")?;
        writeln!(stdout)?;
        for (path, r#type) in &self.status.conflicts {
            let status = format!(
                "{:width$}",
                CONFLICT_LONG_STATUS[r#type],
                width = CONFLICT_LABEL_WIDTH
            );
            writeln!(stdout, "{}", format!("\t{}{}", status, path).red())?;
        }

        Ok(())
    }

    fn print_untracked_files(&self) -> Result<()> {
        if self.status.untracked_files.is_empty() {
            return Ok(());
        }

        let mut stdout = self.ctx.stdout.borrow_mut();

        writeln!(stdout, "Untracked files:")?;
        writeln!(stdout)?;
        for path in &self.status.untracked_files {
            writeln!(stdout, "{}", format!("\t{}", path).red())?;
        }
        writeln!(stdout)?;

        Ok(())
    }

    fn print_commit_status(&self) -> Result<()> {
        if !self.status.index_changes.is_empty() {
            return Ok(());
        }

        let mut stdout = self.ctx.stdout.borrow_mut();

        if !self.status.workspace_changes.is_empty() {
            writeln!(stdout, "no changes added to commit")?;
        } else if !self.status.untracked_files.is_empty() {
            writeln!(
                stdout,
                "nothing added to commit but untracked files present"
            )?;
        } else {
            writeln!(stdout, "nothing to commit, working tree clean")?;
        }

        Ok(())
    }

    fn status_for(&self, path: &str) -> String {
        if self.status.conflicts.contains_key(path) {
            CONFLICT_SHORT_STATUS[&self.status.conflicts[path]].to_owned()
        } else {
            let left = match self.status.index_changes.get(path) {
                Some(change) => SHORT_STATUS[change],
                None => " ",
            };
            let right = match self.status.workspace_changes.get(path) {
                Some(change) => SHORT_STATUS[change],
                None => " ",
            };

            left.to_owned() + right
        }
    }
}
