use crate::commands::CommandContext;
use crate::errors::Result;
use crate::repository::{ChangeType, Repository};
use colored::Colorize;
use lazy_static::lazy_static;
use std::collections::{BTreeMap, HashMap, VecDeque};

pub struct Status {
    repo: Repository,
    argv: VecDeque<String>,
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

impl Status {
    pub fn new(ctx: CommandContext) -> Self {
        Self {
            repo: ctx.repo,
            argv: ctx.argv,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        self.repo.index.load_for_update()?;
        self.repo.initialize_status()?;
        self.repo.index.write_updates()?;

        self.print_results();

        Ok(())
    }

    fn print_results(&self) {
        if self.argv.contains(&String::from("--porcelain")) {
            self.print_porcelain_format();
        } else {
            self.print_long_format();
        }
    }

    fn print_porcelain_format(&self) {
        for path in &self.repo.changed {
            let status = self.status_for(&path);
            println!("{} {}", status, path);
        }
        for path in &self.repo.untracked_files {
            println!("?? {}", path);
        }
    }

    fn print_long_format(&self) {
        self.print_changeset("Changes to be committed", &self.repo.index_changes, "green");
        self.print_changeset(
            "Changes not staged for commit",
            &self.repo.workspace_changes,
            "red",
        );
        self.print_untracked_files();

        self.print_commit_status();
    }

    fn print_changeset(
        &self,
        message: &str,
        changeset: &BTreeMap<String, ChangeType>,
        style: &str,
    ) {
        if changeset.is_empty() {
            return;
        }

        println!("{}:", message);
        println!();
        for (path, change_type) in changeset {
            let status = format!("{:width$}", LONG_STATUS[change_type], width = LABEL_WIDTH);
            println!("{}", format!("\t{}{}", status, path).color(style));
        }
        println!();
    }

    fn print_untracked_files(&self) {
        if self.repo.untracked_files.is_empty() {
            return;
        }

        println!("Untracked files:");
        println!();
        for path in &self.repo.untracked_files {
            println!("{}", format!("\t{}", path).red());
        }
        println!();
    }

    fn print_commit_status(&self) {
        if !self.repo.index_changes.is_empty() {
            return;
        }

        if !self.repo.workspace_changes.is_empty() {
            println!("no changes added to commit");
        } else if !self.repo.untracked_files.is_empty() {
            println!("nothing added to commit but untracked files present");
        } else {
            println!("nothing to commit, working tree clean");
        }
    }

    fn status_for(&self, path: &str) -> String {
        let left = match self.repo.index_changes.get(path) {
            Some(change) => SHORT_STATUS[change],
            None => " ",
        };
        let right = match self.repo.workspace_changes.get(path) {
            Some(change) => SHORT_STATUS[change],
            None => " ",
        };

        left.to_owned() + right
    }
}
