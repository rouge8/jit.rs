use std::path::PathBuf;

use crate::commands::shared::commit_writer::{CommitWriter, CONFLICT_MESSAGE};
use crate::commands::{Command, CommandContext};
use crate::database::tree_diff::Differ;
use crate::database::Database;
use crate::editor::Editor;
use crate::errors::{Error, Result};
use crate::merge::inputs::Inputs;
use crate::merge::resolve::Resolve;
use crate::refs::ORIG_HEAD;
use crate::repository::pending_commit::{PendingCommit, PendingCommitType};
use crate::revision::HEAD;

const COMMIT_NOTES: &str = "\
Please enter a commit message to explain why this merge is necessary,
especially if it merges an updated upstream into a topic branch.

Lines starting with '#' will be ignored, and an empty message aborts
the commit.\n";

enum Mode {
    Run,
    Abort,
    Continue,
}

pub struct Merge<'a> {
    ctx: CommandContext<'a>,
    args: Vec<String>,
    message: Option<String>,
    file: Option<PathBuf>,
    edit: bool,
    mode: Mode,
}

impl<'a> Merge<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Result<Self> {
        let (args, mode, message, file, edit) = match &ctx.opt.cmd {
            Command::Merge {
                args,
                abort,
                r#continue,
                message,
                file,
                edit,
                no_edit,
            } => {
                let mode = if *abort {
                    Mode::Abort
                } else if *r#continue {
                    Mode::Continue
                } else {
                    Mode::Run
                };
                (
                    args,
                    mode,
                    message.as_ref().map(|m| m.to_owned()),
                    file.as_ref().map(|f| f.to_owned()),
                    *edit || !*no_edit && message.is_none() && file.is_none(),
                )
            }
            _ => unreachable!(),
        };

        Ok(Self {
            ctx,
            args: args.to_owned(),
            message,
            file,
            edit,
            mode,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        if matches!(self.mode, Mode::Abort) {
            self.handle_abort()?;
        } else if matches!(self.mode, Mode::Continue) {
            self.handle_continue()?;
        }

        let pending_commit = self.commit_writer().pending_commit;
        if pending_commit.in_progress() {
            self.handle_in_progress_merge()?;
        }

        let inputs = Inputs::new(&self.ctx.repo, HEAD.to_string(), self.args[0].clone())?;
        self.ctx.repo.refs.update_ref(ORIG_HEAD, &inputs.left_oid)?;

        if inputs.already_merged() {
            self.handle_merged_ancestor()?;
        }
        if inputs.is_fast_forward() {
            self.handle_fast_forward(&inputs)?;
        }

        pending_commit.start(&inputs.right_oid, PendingCommitType::Merge)?;
        self.resolve_merge(&inputs, &pending_commit)?;
        self.commit_merge(&inputs, &pending_commit)?;

        Ok(())
    }

    fn resolve_merge(&mut self, inputs: &Inputs, pending_commit: &PendingCommit) -> Result<()> {
        self.ctx.repo.index.load_for_update()?;

        let mut merge = Resolve::new(&mut self.ctx.repo, inputs);
        // While not ideal, it's safe to use `println!()` here because `jit merge` doesn't use a
        // pager. Ideally this would be a closure using `self.ctx.stdout` and `writeln!()`, but I
        // couldn't figure out how to get that to work.
        merge.on_progress = |info| println!("{}", info);
        merge.execute()?;

        self.ctx.repo.index.write_updates()?;
        if self.ctx.repo.index.has_conflict() {
            self.fail_on_conflict(inputs, pending_commit)?;
        }

        Ok(())
    }

    fn fail_on_conflict(&self, inputs: &Inputs, pending_commit: &PendingCommit) -> Result<()> {
        let commit_writer = self.commit_writer();

        let message = commit_writer.read_message(self.message.as_deref(), self.file.as_deref())?;
        let message = if message.is_empty() {
            self.default_commit_message(inputs)
        } else {
            message
        };

        self.ctx
            .edit_file(&pending_commit.message_path, |editor: &mut Editor| {
                editor.write(&message)?;
                editor.write("")?;
                editor.note("Conflicts:")?;
                for name in self.ctx.repo.index.conflict_paths() {
                    editor.note(&format!("\t{}", name))?;
                }
                editor.close();

                Ok(())
            })?;

        let mut stdout = self.ctx.stdout.borrow_mut();
        writeln!(
            stdout,
            "Automatic merge failed; fix conflicts and then commit the result."
        )?;
        Err(Error::Exit(1))
    }

    fn commit_merge(&self, inputs: &Inputs, pending_commit: &PendingCommit) -> Result<()> {
        let commit_writer = self.commit_writer();

        let parents = vec![inputs.left_oid.clone(), inputs.right_oid.clone()];
        let message = self.compose_message(inputs, pending_commit)?;

        commit_writer.write_commit(parents, message.as_deref())?;

        commit_writer
            .pending_commit
            .clear(PendingCommitType::Merge)?;

        Ok(())
    }

    fn compose_message(
        &self,
        inputs: &Inputs,
        pending_commit: &PendingCommit,
    ) -> Result<Option<String>> {
        let commit_writer = self.commit_writer();

        let message = commit_writer.read_message(self.message.as_deref(), self.file.as_deref())?;
        let message = if message.is_empty() {
            self.default_commit_message(inputs)
        } else {
            message
        };

        self.ctx
            .edit_file(&pending_commit.message_path, |editor: &mut Editor| {
                editor.write(&message)?;
                editor.write("")?;
                editor.note(COMMIT_NOTES)?;

                if !self.edit {
                    editor.close();
                }

                Ok(())
            })
    }

    fn default_commit_message(&self, inputs: &Inputs) -> String {
        format!("Merge commit '{}'", inputs.right_name.clone())
    }

    fn handle_merged_ancestor(&self) -> Result<()> {
        let mut stdout = self.ctx.stdout.borrow_mut();

        writeln!(stdout, "Already up to date.")?;

        Err(Error::Exit(0))
    }

    fn handle_fast_forward(&mut self, inputs: &Inputs) -> Result<()> {
        let a = Database::short_oid(&inputs.left_oid);
        let b = Database::short_oid(&inputs.right_oid);

        let mut stdout = self.ctx.stdout.borrow_mut();
        writeln!(stdout, "Updating {}..{}", a, b)?;
        writeln!(stdout, "Fast-forward")?;

        self.ctx.repo.index.load_for_update()?;

        let tree_diff = self.ctx.repo.database.tree_diff(
            Some(&inputs.left_oid),
            Some(&inputs.right_oid),
            None,
        )?;
        self.ctx.repo.migration(tree_diff).apply_changes()?;

        self.ctx.repo.index.write_updates()?;
        self.ctx.repo.refs.update_head(&inputs.right_oid)?;

        Err(Error::Exit(0))
    }

    fn handle_abort(&mut self) -> Result<()> {
        match self
            .ctx
            .repo
            .pending_commit()
            .clear(PendingCommitType::Merge)
        {
            Ok(()) => (),
            Err(err) => {
                let mut stderr = self.ctx.stderr.borrow_mut();
                writeln!(stderr, "fatal: {}", err)?;

                return Err(Error::Exit(128));
            }
        }

        self.ctx.repo.index.load_for_update()?;
        self.ctx
            .repo
            .hard_reset(self.ctx.repo.refs.read_head()?.as_ref().unwrap())?;
        self.ctx.repo.index.write_updates()?;

        Err(Error::Exit(0))
    }

    fn handle_continue(&mut self) -> Result<()> {
        self.ctx.repo.index.load()?;

        match self.commit_writer().resume_merge(PendingCommitType::Merge) {
            Ok(()) => Ok(()),
            Err(err) => match err {
                Error::NoMergeInProgress(..) => {
                    let mut stderr = self.ctx.stderr.borrow_mut();
                    writeln!(stderr, "fatal: {}", err)?;

                    Err(Error::Exit(128))
                }
                _ => Err(err),
            },
        }
    }

    fn handle_in_progress_merge(&self) -> Result<()> {
        let mut stderr = self.ctx.stderr.borrow_mut();
        writeln!(
            stderr,
            "error: Merging is not possible because you have unmerged files."
        )?;
        writeln!(stderr, "{}", CONFLICT_MESSAGE)?;

        Err(Error::Exit(128))
    }

    fn commit_writer(&self) -> CommitWriter {
        CommitWriter::new(&self.ctx)
    }
}
