use crate::commands::{Command, CommandContext};
use crate::errors::Result;
use crate::refs::HEAD;
use crate::repository::status::Status as RepositoryStatus;
use crate::repository::ChangeType;
use colored::Colorize;
use lazy_static::lazy_static;
use std::collections::{BTreeMap, HashMap};
use std::io::Write;

pub struct Status<'a> {
    ctx: CommandContext<'a>,
    status: RepositoryStatus,
    /// `jit status --porcelain`
    porcelain: bool,
}

lazy_static! {
    static ref SHORT_STATUS: HashMap<ChangeType, &'static str> = {
        let mut m = HashMap::new();
        m.insert(ChangeType::Added, "A");
        m.insert(ChangeType::Deleted, "D");
        m.insert(ChangeType::Modified, "M");
        m
    };
    static ref LONG_STATUS: HashMap<ChangeType, &'static str> = {
        let mut m = HashMap::new();
        m.insert(ChangeType::Added, "new file:");
        m.insert(ChangeType::Deleted, "deleted:");
        m.insert(ChangeType::Modified, "modified:");
        m
    };
    static ref CONFLICT_SHORT_STATUS: HashMap<Vec<u16>, &'static str> = {
        let mut m = HashMap::new();
        m.insert(vec![1, 2, 3], "UU");
        m.insert(vec![1, 2], "UD");
        m.insert(vec![1, 3], "DU");
        m.insert(vec![2, 3], "AA");
        m.insert(vec![2], "AU");
        m.insert(vec![3], "UA");
        m
    };
    static ref CONFLICT_LONG_STATUS: HashMap<Vec<u16>, &'static str> = {
        let mut m = HashMap::new();
        m.insert(vec![1, 2, 3], "both modified:");
        m.insert(vec![1, 2], "deleted by them:");
        m.insert(vec![1, 3], "deleted by us:");
        m.insert(vec![2, 3], "both added:");
        m.insert(vec![2], "added by us:");
        m.insert(vec![3], "added by them:");
        m
    };
}

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
