use crate::commands::shared::commit_writer::CommitWriter;
use crate::commands::{Command, CommandContext};
use crate::database::commit::Commit as DatabaseCommit;
use crate::database::object::Object;
use crate::editor::Editor;
use crate::errors::{Error, Result};
use crate::revision::{Revision, COMMIT};
use std::path::PathBuf;

pub const COMMIT_NOTES: &str = "\
Please enter the commit message for your changes. Lines starting
with # will be ignored, and an empty message aborts the commit.\n";

pub struct Commit<'a> {
    ctx: CommandContext<'a>,
    message: Option<String>,
    file: Option<PathBuf>,
    edit: bool,
    reuse: Option<String>,
    amend: bool,
}

impl<'a> Commit<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Self {
        let (message, file, edit, reuse, amend) = match &ctx.opt.cmd {
            Command::Commit {
                message,
                file,
                edit,
                no_edit,
                reuse_message,
                reedit_message,
                amend,
            } => (
                message.as_ref().map(|m| m.to_owned()),
                file.as_ref().map(|f| f.to_owned()),
                *edit
                    || !*no_edit && message.is_none() && file.is_none()
                    || reedit_message.is_some(),
                reedit_message
                    .to_owned()
                    .or_else(|| reuse_message.to_owned()),
                *amend,
            ),
            _ => unreachable!(),
        };

        Self {
            ctx,
            message,
            file,
            edit,
            reuse,
            amend,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        self.ctx.repo.index.load()?;

        if self.amend {
            self.handle_amend()?;
        }

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
        let message = if message.is_empty() {
            self.reused_message()?.unwrap_or_else(String::new)
        } else {
            message
        };
        let message = self.compose_message(&message)?;

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

    fn reused_message(&self) -> Result<Option<String>> {
        if let Some(reuse) = &self.reuse {
            let mut revision = Revision::new(&self.ctx.repo, reuse);
            let commit = self
                .ctx
                .repo
                .database
                .load_commit(&revision.resolve(Some(COMMIT))?)?;

            Ok(Some(commit.message))
        } else {
            Ok(None)
        }
    }

    fn handle_amend(&self) -> Result<()> {
        let old = self
            .ctx
            .repo
            .database
            .load_commit(&self.ctx.repo.refs.read_head()?.expect("nothing to amend"))?;

        let commit_writer = self.commit_writer();
        let tree = commit_writer.write_tree();

        let message = commit_writer.read_message(self.message.as_deref(), self.file.as_deref())?;
        let message = if message.is_empty() {
            old.message.clone()
        } else {
            message
        };
        let message = self.compose_message(&message)?;

        let committer = commit_writer.current_author();

        let new = DatabaseCommit::new(
            old.parents.clone(),
            tree.oid(),
            old.author,
            committer,
            message.unwrap_or_else(String::new),
        );
        self.ctx.repo.database.store(&new)?;
        self.ctx.repo.refs.update_head(&new.oid())?;

        commit_writer.print_commit(&new)?;

        Err(Error::Exit(0))
    }
}
