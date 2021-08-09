use crate::commands::shared::commit_writer::CommitWriter;
use crate::commands::{Command, CommandContext};
use crate::errors::Result;
use std::path::PathBuf;

pub struct Commit<'a> {
    ctx: CommandContext<'a>,
    message: Option<String>,
    file: Option<PathBuf>,
}

impl<'a> Commit<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Self {
        let (message, file) = match &ctx.opt.cmd {
            Command::Commit { message, file } => (
                message.as_ref().map(|m| m.to_owned()),
                file.as_ref().map(|f| f.to_owned()),
            ),
            _ => unreachable!(),
        };

        Self { ctx, message, file }
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
        let message = commit_writer.read_message(self.message.as_deref(), self.file.as_deref())?;
        let commit = commit_writer.write_commit(parents, &message)?;

        commit_writer.print_commit(&commit)?;

        Ok(())
    }

    fn commit_writer(&self) -> CommitWriter {
        CommitWriter::new(&self.ctx)
    }
}
