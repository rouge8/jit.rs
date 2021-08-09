use crate::commands::shared::commit_writer::CommitWriter;
use crate::commands::CommandContext;
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

        let commit_writer = self.commit_writer();
        if commit_writer.pending_commit.in_progress() {
            commit_writer.resume_merge()?;
        }

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

        let commit = commit_writer.write_commit(parents, &message)?;

        commit_writer.print_commit(&commit)?;

        Ok(())
    }

    fn commit_writer(&self) -> CommitWriter {
        CommitWriter::new(&self.ctx)
    }
}
