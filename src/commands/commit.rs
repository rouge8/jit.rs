use crate::commands::CommandContext;
use crate::database::author::Author;
use crate::database::commit::Commit as DatabaseCommit;
use crate::database::entry::Entry;
use crate::database::object::Object;
use crate::database::tree::Tree;
use crate::errors::Error;
use crate::errors::Result;
use chrono::Local;
use std::io;
use std::io::Read;

pub struct Commit;

impl Commit {
    pub fn run(mut ctx: CommandContext) -> Result<()> {
        ctx.repo.index.load()?;

        let entries = ctx.repo.index.entries.values().map(Entry::from).collect();
        let root = Tree::build(entries);
        root.traverse(&|tree| {
            ctx.repo.database.store(tree).unwrap();
        });

        let parent = ctx.repo.refs.read_head()?;
        let name = &ctx.env["GIT_AUTHOR_NAME"];
        let email = &ctx.env["GIT_AUTHOR_EMAIL"];
        let author = Author::new(name.clone(), email.clone(), Local::now());
        let mut message = String::new();
        io::stdin().read_to_string(&mut message)?;

        message = message.trim().to_string();
        if message.is_empty() {
            eprintln!("Aborting commit due to empty commit message.");
            return Err(Error::Exit(0));
        }

        let commit = DatabaseCommit::new(parent, root.oid(), author, message);
        ctx.repo.database.store(&commit)?;
        ctx.repo.refs.update_head(commit.oid())?;

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
