use crate::commands::shared::commit_writer::{CommitWriter, CONFLICT_MESSAGE};
use crate::commands::{Command, CommandContext};
use crate::database::tree_diff::Differ;
use crate::database::Database;
use crate::errors::{Error, Result};
use crate::merge::inputs::Inputs;
use crate::merge::resolve::Resolve;
use crate::refs::ORIG_HEAD;
use crate::revision::HEAD;
use std::path::PathBuf;

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
    mode: Mode,
}

impl<'a> Merge<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Result<Self> {
        let (args, mode, message, file) = match &ctx.opt.cmd {
            Command::Merge {
                args,
                abort,
                r#continue,
                message,
                file,
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
                )
            }
            _ => unreachable!(),
        };

        Ok(Self {
            ctx,
            args: args.to_owned(),
            message,
            file,
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

        pending_commit.start(
            &inputs.right_oid,
            &self
                .commit_writer()
                .read_message(self.message.as_deref(), self.file.as_deref())?,
        )?;
        self.resolve_merge(&inputs)?;
        self.commit_merge(&inputs)?;

        Ok(())
    }

    fn resolve_merge(&mut self, inputs: &Inputs) -> Result<()> {
        self.ctx.repo.index.load_for_update()?;

        let mut merge = Resolve::new(&mut self.ctx.repo, inputs);
        // While not ideal, it's safe to use `println!()` here because `jit merge` doesn't use a
        // pager. Ideally this would be a closure using `self.ctx.stdout` and `writeln!()`, but I
        // couldn't figure out how to get that to work.
        merge.on_progress = |info| println!("{}", info);
        merge.execute()?;

        self.ctx.repo.index.write_updates()?;
        if self.ctx.repo.index.has_conflict() {
            let mut stdout = self.ctx.stdout.borrow_mut();
            writeln!(
                stdout,
                "Automatic merge failed; fix conflicts and then commit the result."
            )?;
            return Err(Error::Exit(1));
        }

        Ok(())
    }

    fn commit_merge(&self, inputs: &Inputs) -> Result<()> {
        let commit_writer = self.commit_writer();

        let parents = vec![inputs.left_oid.clone(), inputs.right_oid.clone()];
        let message = &commit_writer.pending_commit.merge_message()?;

        commit_writer.write_commit(parents, message)?;

        commit_writer.pending_commit.clear()?;

        Ok(())
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
        match self.ctx.repo.pending_commit().clear() {
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

        match self.commit_writer().resume_merge() {
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
