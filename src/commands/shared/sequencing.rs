use crate::commands::shared::commit_writer::CommitWriter;
use crate::commands::CommandContext;
use crate::database::commit::Commit;
use crate::database::object::Object;
use crate::editor::Editor;
use crate::errors::{Error, Result};
use crate::merge::inputs;
use crate::merge::resolve::Resolve;
use crate::repository::pending_commit::PendingCommitType;
use crate::repository::sequencer::{Action, Sequencer};
use crate::repository::Repository;

const CONFLICT_NOTES: &str = "\
after resolving the conflicts, mark the corrected paths
with 'jit add <paths>' or 'jit rm <paths>'
and commit the result with 'jit commit'";

pub enum Mode {
    Run,
    Continue,
    Abort,
    Quit,
}

pub fn resolve_merge(repo: &mut Repository, inputs: &inputs::CherryPick) -> Result<()> {
    repo.index.load_for_update()?;
    Resolve::new(repo, inputs).execute()?;
    repo.index.write_updates()?;

    Ok(())
}

pub fn fail_on_conflict(
    ctx: &CommandContext,
    commit_writer: &CommitWriter,
    sequencer: &mut Sequencer,
    inputs: &inputs::CherryPick,
    merge_type: PendingCommitType,
    message: &str,
) -> Result<()> {
    sequencer.dump()?;

    commit_writer
        .pending_commit
        .start(&inputs.right_oid, merge_type)?;

    ctx.edit_file(
        &commit_writer.pending_commit.message_path,
        |editor: &mut Editor| {
            editor.write(message)?;
            editor.write("")?;
            editor.note("Conflicts:")?;
            for name in ctx.repo.index.conflict_paths() {
                editor.note(&format!("\t{}", name))?;
            }
            editor.close();

            Ok(())
        },
    )?;

    let mut stderr = ctx.stderr.borrow_mut();
    writeln!(stderr, "error: could not apply {}", inputs.right_name)?;
    for line in CONFLICT_NOTES.lines() {
        writeln!(stderr, "hint: {}", line)?;
    }

    Err(Error::Exit(1))
}

pub fn finish_commit(
    repo: &Repository,
    commit_writer: &CommitWriter,
    commit: &Commit,
) -> Result<()> {
    repo.database.store(commit)?;
    repo.refs.update_head(&commit.oid())?;
    commit_writer.print_commit(commit)?;

    Ok(())
}

pub fn resume_sequencer(
    sequencer: &mut Sequencer,
    pick: &mut dyn FnMut(&mut Sequencer, &Commit) -> Result<()>,
    revert: &mut dyn FnMut(&mut Sequencer, &Commit) -> Result<()>,
) -> Result<()> {
    while let Some((action, commit)) = sequencer.next_command() {
        match action {
            Action::Pick => pick(sequencer, &commit)?,
            Action::Revert => revert(sequencer, &commit)?,
        }
        sequencer.drop_command()?;
    }

    sequencer.quit()?;
    Err(Error::Exit(0))
}

pub fn handle_abort(
    ctx: &CommandContext,
    commit_writer: &CommitWriter,
    sequencer: &mut Sequencer,
    merge_type: PendingCommitType,
) -> Result<()> {
    let pending_commit = &commit_writer.pending_commit;
    if pending_commit.in_progress() {
        pending_commit.clear(merge_type)?;
    }
    // sequencer.abort() calls repo.hard_reset() which updates the in-memory index on
    // `sequencer.repo`, not `self.ctx.repo`.
    sequencer.repo.index.load_for_update()?;

    match sequencer.abort() {
        Ok(()) => (),
        Err(err) => {
            let mut stderr = ctx.stderr.borrow_mut();
            writeln!(stderr, "warning: {}", err)?;
        }
    }

    sequencer.repo.index.write_updates()?;

    Err(Error::Exit(0))
}

pub fn handle_quit(
    commit_writer: &CommitWriter,
    sequencer: &mut Sequencer,
    merge_type: PendingCommitType,
) -> Result<()> {
    let pending_commit = &commit_writer.pending_commit;
    if pending_commit.in_progress() {
        pending_commit.clear(merge_type)?;
    }
    sequencer.quit()?;

    Ok(())
}
