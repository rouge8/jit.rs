use crate::commands::commit::COMMIT_NOTES;
use crate::commands::shared::commit_writer::CommitWriter;
use crate::commands::shared::sequencing::{
    fail_on_conflict, finish_commit, handle_abort, handle_quit, resolve_merge, resume_sequencer,
    Mode,
};
use crate::commands::{Command, CommandContext};
use crate::database::commit::Commit;
use crate::database::object::Object;
use crate::database::Database;
use crate::errors::{Error, Result};
use crate::merge::inputs;
use crate::refs::HEAD;
use crate::repository::pending_commit::PendingCommitType;
use crate::repository::sequencer::Sequencer;
use crate::rev_list::{RevList, RevListOptions};

pub struct Revert<'a> {
    ctx: CommandContext<'a>,
    args: Vec<String>,
    mode: Mode,
}

impl<'a> Revert<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Self {
        let (args, mode) = match &ctx.opt.cmd {
            Command::Revert {
                args,
                r#continue,
                abort,
                quit,
            } => (
                args.to_owned(),
                if *r#continue {
                    Mode::Continue
                } else if *abort {
                    Mode::Abort
                } else if *quit {
                    Mode::Quit
                } else {
                    Mode::Run
                },
            ),
            _ => unreachable!(),
        };

        Self { ctx, args, mode }
    }

    pub fn run(&mut self) -> Result<()> {
        let mut sequencer = Sequencer::new(&self.ctx.repo);
        let commit_writer = self.commit_writer();

        match self.mode {
            Mode::Continue => self.handle_continue(&mut sequencer)?,
            Mode::Abort => handle_abort(
                &self.ctx,
                &commit_writer,
                &mut sequencer,
                PendingCommitType::Revert,
            )?,
            Mode::Quit => handle_quit(&commit_writer, &mut sequencer, PendingCommitType::Revert)?,
            Mode::Run => {
                sequencer.start()?;
                self.store_commit_sequence(&mut sequencer)?;
                resume_sequencer(
                    &mut sequencer,
                    &mut |_sequencer, _commit| unimplemented!(),
                    &mut |sequencer, commit| self.revert(sequencer, commit),
                )?;
            }
        }

        Ok(())
    }

    fn store_commit_sequence(&self, sequencer: &mut Sequencer) -> Result<()> {
        let args: Vec<_> = self.args.iter().map(|s| s.to_owned()).collect();
        let commits: Vec<_> =
            RevList::new(&self.ctx.repo, &args, RevListOptions { walk: false })?.collect();
        for commit in commits.iter() {
            sequencer.revert(commit);
        }

        Ok(())
    }

    fn revert(&mut self, sequencer: &mut Sequencer, commit: &Commit) -> Result<()> {
        let inputs = self.revert_merge_inputs(commit)?;
        let message = self.revert_commit_message(commit);

        resolve_merge(&mut self.ctx.repo, &inputs)?;

        let commit_writer = self.commit_writer();

        if self.ctx.repo.index.has_conflict() {
            fail_on_conflict(
                &self.ctx,
                &commit_writer,
                sequencer,
                &inputs,
                PendingCommitType::Revert,
                &message,
            )?;
        }

        let author = commit_writer.current_author();
        let message = self.edit_revert_message(&message)?.unwrap();
        let picked = Commit::new(
            vec![inputs.left_oid],
            commit_writer.write_tree().oid(),
            author.clone(),
            author,
            message,
        );

        finish_commit(&self.ctx.repo, &commit_writer, &picked)?;

        Ok(())
    }

    fn revert_merge_inputs(&self, commit: &Commit) -> Result<inputs::CherryPick> {
        let short = Database::short_oid(&commit.oid());

        let left_name = HEAD.to_owned();
        let left_oid = self.ctx.repo.refs.read_head()?.unwrap();

        let right_name = format!("parent of {}... {}", short, commit.title_line().trim());
        let right_oid = commit.parent().unwrap();

        Ok(inputs::CherryPick::new(
            left_name,
            right_name,
            left_oid,
            right_oid,
            vec![commit.oid()],
        ))
    }

    fn revert_commit_message(&self, commit: &Commit) -> String {
        format!(
            "\
Revert \"{}\"

This reverts commit {}.
",
            commit.title_line().trim(),
            commit.oid()
        )
    }

    fn edit_revert_message(&self, message: &str) -> Result<Option<String>> {
        self.ctx
            .edit_file(&self.commit_writer().commit_message_path(), |editor| {
                editor.write(message)?;
                editor.write("")?;
                editor.note(COMMIT_NOTES)?;

                Ok(())
            })
    }

    fn handle_continue(&mut self, sequencer: &mut Sequencer) -> Result<()> {
        self.ctx.repo.index.load()?;

        if self.commit_writer().pending_commit.in_progress() {
            match self.commit_writer().write_revert_commit() {
                Ok(()) => (),
                Err(err) => match err {
                    Error::NoMergeInProgress(..) => {
                        let mut stderr = self.ctx.stderr.borrow_mut();
                        writeln!(stderr, "fatal: {}", err)?;

                        return Err(Error::Exit(128));
                    }
                    _ => return Err(err),
                },
            }
        }

        sequencer.load()?;
        sequencer.drop_command()?;
        resume_sequencer(
            sequencer,
            &mut |_sequencer, _commit| unimplemented!(),
            &mut |sequencer, commit| self.revert(sequencer, commit),
        )?;

        Ok(())
    }

    fn commit_writer(&self) -> CommitWriter {
        CommitWriter::new(&self.ctx)
    }
}
