use crate::commands::CommandContext;
use crate::database::author::Author;
use crate::database::commit::Commit as DatabaseCommit;
use crate::database::entry::Entry;
use crate::database::object::Object;
use crate::database::tree::Tree;
use crate::errors::Error;
use crate::errors::Result;
use chrono::{DateTime, Local};
use std::io;
use std::io::{Read, Write};

pub struct Commit<'a> {
    ctx: CommandContext<'a>,
}

impl<'a> Commit<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Self {
        Self { ctx }
    }

    pub fn run(&mut self) -> Result<()> {
        self.ctx.repo.index.load()?;

        let entries = self
            .ctx
            .repo
            .index
            .entries
            .values()
            .map(Entry::from)
            .collect();
        let root = Tree::build(entries);
        root.traverse(&|tree| {
            self.ctx.repo.database.store(tree).unwrap();
        });

        let parents = if let Some(parent) = self.ctx.repo.refs.read_head()? {
            vec![parent]
        } else {
            vec![]
        };
        let name = &self.ctx.env["GIT_AUTHOR_NAME"];
        let email = &self.ctx.env["GIT_AUTHOR_EMAIL"];
        let author_date = if let Some(author_date_str) = self.ctx.env.get("GIT_AUTHOR_DATE") {
            DateTime::parse_from_rfc2822(author_date_str).expect("could not parse GIT_AUTHOR_DATE")
        } else {
            let now = Local::now();
            now.with_timezone(now.offset())
        };
        let author = Author::new(name.clone(), email.clone(), author_date);
        let mut message = String::new();
        io::stdin().read_to_string(&mut message)?;

        message = message.trim().to_string();
        if message.is_empty() {
            let mut stderr = self.ctx.stderr.borrow_mut();
            writeln!(stderr, "Aborting commit due to empty commit message.")?;
            return Err(Error::Exit(0));
        }

        let commit = DatabaseCommit::new(parents, root.oid(), author, message);
        self.ctx.repo.database.store(&commit)?;
        self.ctx.repo.refs.update_head(commit.oid())?;

        let mut is_root = String::new();
        match commit.parent() {
            Some(_) => (),
            None => is_root.push_str("(root-commit) "),
        }

        let mut stdout = self.ctx.stdout.borrow_mut();
        writeln!(
            stdout,
            "[{}{}] {}",
            is_root,
            commit.oid(),
            commit.message.lines().next().unwrap(),
        )?;

        Ok(())
    }
}
