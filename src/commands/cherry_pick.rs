use crate::commands::shared::commit_writer::CommitWriter;
use crate::commands::{Command, CommandContext};
use crate::database::commit::Commit;
use crate::database::object::Object;
use crate::database::Database;
use crate::editor::Editor;
use crate::errors::{Error, Result};
use crate::merge::inputs;
use crate::merge::resolve::Resolve;
use crate::refs::HEAD;
use crate::repository::pending_commit::PendingCommitType;
use crate::revision::{Revision, COMMIT};

const CONFLICT_NOTES: &str = "\
after resolving the conflicts, mark the corrected paths
with 'jit add <paths>' or 'jit rm <paths>'
and commit the result with 'jit commit'";

enum Mode {
    Run,
    Continue,
}

pub struct CherryPick<'a> {
    ctx: CommandContext<'a>,
    args: Vec<String>,
    mode: Mode,
}

impl<'a> CherryPick<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Self {
        let (args, mode) = match &ctx.opt.cmd {
            Command::CherryPick { args, r#continue } => (
                args.to_owned(),
                if *r#continue {
                    Mode::Continue
                } else {
                    Mode::Run
                },
            ),
            _ => unreachable!(),
        };

        Self { ctx, args, mode }
    }

    pub fn run(&mut self) -> Result<()> {
        if matches!(self.mode, Mode::Continue) {
            self.handle_continue()?;
        }

        let mut revision = Revision::new(&self.ctx.repo, &self.args[0]);
        let commit = self
            .ctx
            .repo
            .database
            .load_commit(&revision.resolve(Some(COMMIT))?)?;

        self.pick(&commit)?;

        Ok(())
    }

    fn pick(&mut self, commit: &Commit) -> Result<()> {
        let inputs = self.pick_merge_inputs(commit)?;

        self.resolve_merge(&inputs)?;

        let commit_writer = self.commit_writer();

        if self.ctx.repo.index.has_conflict() {
            self.fail_on_conflict(&commit_writer, &inputs, &commit.message)?;
        }

        let picked = Commit::new(
            vec![inputs.left_oid],
            commit_writer.write_tree().oid(),
            commit.author.clone(),
            commit_writer.current_author(),
            commit.message.clone(),
        );

        self.finish_commit(&commit_writer, &picked)?;

        Ok(())
    }

    fn pick_merge_inputs(&self, commit: &Commit) -> Result<inputs::CherryPick> {
        let short = Database::short_oid(&commit.oid());

        let left_name = HEAD.to_owned();
        let left_oid = self.ctx.repo.refs.read_head()?.unwrap();

        let right_name = format!("{}... {}", short, commit.title_line().trim());
        let right_oid = commit.oid();

        Ok(inputs::CherryPick::new(
            left_name,
            right_name,
            left_oid,
            right_oid,
            vec![commit.parent().unwrap()],
        ))
    }

    fn resolve_merge(&mut self, inputs: &inputs::CherryPick) -> Result<()> {
        self.ctx.repo.index.load_for_update()?;
        Resolve::new(&mut self.ctx.repo, inputs).execute()?;
        self.ctx.repo.index.write_updates()?;

        Ok(())
    }

    fn fail_on_conflict(
        &self,
        commit_writer: &CommitWriter,
        inputs: &inputs::CherryPick,
        message: &str,
    ) -> Result<()> {
        commit_writer
            .pending_commit
            .start(&inputs.right_oid, PendingCommitType::CherryPick)?;

        self.ctx.edit_file(
            &commit_writer.pending_commit.message_path,
            |editor: &mut Editor| {
                editor.write(message)?;
                editor.write("")?;
                editor.note("Conflicts:")?;
                for name in self.ctx.repo.index.conflict_paths() {
                    editor.note(&format!("\t{}", name))?;
                }
                editor.close();

                Ok(())
            },
        )?;

        let mut stderr = self.ctx.stderr.borrow_mut();
        writeln!(stderr, "error: could not apply {}", inputs.right_name)?;
        for line in CONFLICT_NOTES.lines() {
            writeln!(stderr, "hint: {}", line)?;
        }

        Err(Error::Exit(1))
    }

    fn finish_commit(&self, commit_writer: &CommitWriter, commit: &Commit) -> Result<()> {
        self.ctx.repo.database.store(commit)?;
        self.ctx.repo.refs.update_head(&commit.oid())?;
        commit_writer.print_commit(commit)?;

        Ok(())
    }

    fn handle_continue(&mut self) -> Result<()> {
        self.ctx.repo.index.load()?;

        match self.commit_writer().write_cherry_pick_commit() {
            Ok(()) => Err(Error::Exit(0)),
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

    fn commit_writer(&self) -> CommitWriter {
        CommitWriter::new(&self.ctx)
    }
}
