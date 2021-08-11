use crate::commands::shared::commit_writer::CommitWriter;
use crate::commands::{Command, CommandContext};
use crate::editor::Editor;
use crate::errors::Result;
use std::path::PathBuf;

const COMMIT_NOTES: &str = "Please enter the commit message for yhour changes. Lines starting
with # will be ignored, and an empty message aborts the commit.\n";

pub struct Commit<'a> {
    ctx: CommandContext<'a>,
    message: Option<String>,
    file: Option<PathBuf>,
    edit: bool,
}

impl<'a> Commit<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Self {
        let (message, file, edit) = match &ctx.opt.cmd {
            Command::Commit {
                message,
                file,
                edit,
                no_edit,
            } => (
                message.as_ref().map(|m| m.to_owned()),
                file.as_ref().map(|f| f.to_owned()),
                *edit || !*no_edit && message.is_none() && file.is_none(),
            ),
            _ => unreachable!(),
        };

        Self {
            ctx,
            message,
            file,
            edit,
        }
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
        let message = self.compose_message(
            &commit_writer.read_message(self.message.as_deref(), self.file.as_deref())?,
        )?;
        let commit = commit_writer.write_commit(parents, message.as_deref())?;

        commit_writer.print_commit(&commit)?;

        Ok(())
    }

    fn commit_writer(&self) -> CommitWriter {
        CommitWriter::new(&self.ctx)
    }

    fn compose_message(&self, message: &str) -> Result<Option<String>> {
        self.ctx.edit_file(
            &self.commit_writer().commit_message_path(),
            |editor: &mut Editor| {
                editor.write(message)?;
                editor.write("")?;
                editor.note(COMMIT_NOTES)?;

                if !self.edit {
                    editor.close();
                }

                Ok(())
            },
        )
    }
}
