use crate::commands::shared::write_commit::WriteCommit;
use crate::commands::CommandContext;
use crate::database::object::Object;
use crate::errors::Error;
use crate::errors::Result;
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

        let parents = if let Some(parent) = self.ctx.repo.refs.read_head()? {
            vec![parent]
        } else {
            vec![]
        };
        let mut message = String::new();
        io::stdin().read_to_string(&mut message)?;

        message = message.trim().to_string();
        if message.is_empty() {
            let mut stderr = self.ctx.stderr.borrow_mut();
            writeln!(stderr, "Aborting commit due to empty commit message.")?;
            return Err(Error::Exit(0));
        }

        let commit = self.write_commit().write_commit(parents, message)?;

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

    fn write_commit(&self) -> WriteCommit {
        WriteCommit::new(&self.ctx)
    }
}
