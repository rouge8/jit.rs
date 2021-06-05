use crate::commands::CommandContext;
use crate::database::author::Author;
use crate::database::commit::Commit as DatabaseCommit;
use crate::database::entry::Entry;
use crate::database::object::Object;
use crate::database::tree::Tree;
use crate::errors::Error;
use crate::errors::Result;
use crate::repository::Repository;
use chrono::Local;
use std::collections::HashMap;
use std::io;
use std::io::Read;

pub struct Commit {
    repo: Repository,
    env: HashMap<String, String>,
}

impl Commit {
    pub fn new(ctx: CommandContext) -> Self {
        Self {
            repo: ctx.repo,
            env: ctx.env,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        self.repo.index.load()?;

        let entries = self.repo.index.entries.values().map(Entry::from).collect();
        let root = Tree::build(entries);
        root.traverse(&|tree| {
            self.repo.database.store(tree).unwrap();
        });

        let parent = self.repo.refs.read_head()?;
        let name = &self.env["GIT_AUTHOR_NAME"];
        let email = &self.env["GIT_AUTHOR_EMAIL"];
        let now = Local::now();
        let author = Author::new(name.clone(), email.clone(), now.with_timezone(now.offset()));
        let mut message = String::new();
        io::stdin().read_to_string(&mut message)?;

        message = message.trim().to_string();
        if message.is_empty() {
            eprintln!("Aborting commit due to empty commit message.");
            return Err(Error::Exit(0));
        }

        let commit = DatabaseCommit::new(parent, root.oid(), author, message);
        self.repo.database.store(&commit)?;
        self.repo.refs.update_head(commit.oid())?;

        let mut is_root = String::new();
        match commit.parent {
            Some(_) => (),
            None => is_root.push_str("(root-commit) "),
        }
        println!(
            "[{}{}] {}",
            is_root,
            commit.oid(),
            commit.message.lines().next().unwrap(),
        );

        Ok(())
    }
}
