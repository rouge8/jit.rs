use crate::commands::CommandContext;
use crate::errors::Result;
use crate::repository::ChangeType;
use colored::Colorize;
use lazy_static::lazy_static;
use std::collections::{BTreeMap, HashMap};
use std::io::Write;

pub struct Status<E: Write> {
    ctx: CommandContext<E>,
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
}

static LABEL_WIDTH: usize = 12;

impl<E: Write> Status<E> {
    pub fn new(ctx: CommandContext<E>) -> Self {
        Self { ctx }
    }

    pub fn run(&mut self) -> Result<()> {
        self.ctx.repo.index.load_for_update()?;
        self.ctx.repo.initialize_status()?;
        self.ctx.repo.index.write_updates()?;

        self.print_results()?;

        Ok(())
    }

    fn print_results(&self) -> Result<()> {
        if self.ctx.argv.contains(&String::from("--porcelain")) {
            self.print_porcelain_format()?;
        } else {
            self.print_long_format()?;
        }

        Ok(())
    }

    fn print_porcelain_format(&self) -> Result<()> {
        let mut stdout = self.ctx.stdout.borrow_mut();

        for path in &self.ctx.repo.changed {
            let status = self.status_for(&path);
            writeln!(stdout, "{} {}", status, path)?;
        }
        for path in &self.ctx.repo.untracked_files {
            writeln!(stdout, "?? {}", path)?;
        }

        Ok(())
    }

    fn print_long_format(&self) -> Result<()> {
        self.print_changeset(
            "Changes to be committed",
            &self.ctx.repo.index_changes,
            "green",
        )?;
        self.print_changeset(
            "Changes not staged for commit",
            &self.ctx.repo.workspace_changes,
            "red",
        )?;
        self.print_untracked_files()?;

        self.print_commit_status()?;

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

    fn print_untracked_files(&self) -> Result<()> {
        if self.ctx.repo.untracked_files.is_empty() {
            return Ok(());
        }

        let mut stdout = self.ctx.stdout.borrow_mut();

        writeln!(stdout, "Untracked files:")?;
        writeln!(stdout)?;
        for path in &self.ctx.repo.untracked_files {
            writeln!(stdout, "{}", format!("\t{}", path).red())?;
        }
        writeln!(stdout)?;

        Ok(())
    }

    fn print_commit_status(&self) -> Result<()> {
        if !self.ctx.repo.index_changes.is_empty() {
            return Ok(());
        }

        let mut stdout = self.ctx.stdout.borrow_mut();

        if !self.ctx.repo.workspace_changes.is_empty() {
            writeln!(stdout, "no changes added to commit")?;
        } else if !self.ctx.repo.untracked_files.is_empty() {
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
        let left = match self.ctx.repo.index_changes.get(path) {
            Some(change) => SHORT_STATUS[change],
            None => " ",
        };
        let right = match self.ctx.repo.workspace_changes.get(path) {
            Some(change) => SHORT_STATUS[change],
            None => " ",
        };

        left.to_owned() + right
    }
}
