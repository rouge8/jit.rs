use crate::commands::{Command, CommandContext};
use crate::database::{Database, ParsedObject};
use crate::errors::{Error, Result};
use crate::refs::{Ref, HEAD};
use crate::revision::{Revision, COMMIT};
use std::io::Write;

const DETACHED_HEAD_MESSAGE: &str = "\
You are in 'detached HEAD' state. You can look around, make experimental
changes and commit them, and you can discard any commits you make in this
state without impacting any branches by performing another checkout.

If you want to create a new branch to retain commits you create, you may
do so (now or later) by using the branch command. Example:

  jit branch <new-branch-name>\n";

pub struct Checkout<'a> {
    ctx: CommandContext<'a>,
    /// `jit checkout <target>`
    target: String,
}

impl<'a> Checkout<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Self {
        let target = match &ctx.opt.cmd {
            Command::Checkout { tree_ish } => tree_ish.to_owned(),
            _ => unreachable!(),
        };

        Self { ctx, target }
    }

    pub fn run(&mut self) -> Result<()> {
        let current_ref = self.ctx.repo.refs.current_ref(HEAD)?;
        let current_oid = self.ctx.repo.refs.read_oid(&current_ref)?.unwrap();

        let mut revision = Revision::new(&mut self.ctx.repo, &self.target);
        let target_oid = match revision.resolve(Some(COMMIT)) {
            Ok(oid) => oid,
            Err(error) => {
                let mut stderr = self.ctx.stderr.borrow_mut();

                for err in revision.errors {
                    writeln!(stderr, "error: {}", err.message)?;
                    for line in err.hint {
                        writeln!(stderr, "hint: {}", line)?;
                    }
                }
                writeln!(stderr, "error: {}", error)?;

                return Err(Error::Exit(1));
            }
        };

        self.ctx.repo.index.load_for_update()?;

        let tree_diff = self
            .ctx
            .repo
            .database
            .tree_diff(&current_oid, &target_oid)?;
        let mut migration = self.ctx.repo.migration(tree_diff);

        match migration.apply_changes() {
            Ok(()) => (),
            Err(Error::MigrationConflict) => {
                let mut stderr = self.ctx.stderr.borrow_mut();

                for message in migration.errors {
                    writeln!(stderr, "error: {}", message)?;
                }
                writeln!(stderr, "Aborting")?;

                self.ctx.repo.index.release_lock()?;

                return Err(Error::Exit(1));
            }
            Err(err) => return Err(err),
        }

        self.ctx.repo.index.write_updates()?;
        self.ctx.repo.refs.set_head(&self.target, &target_oid)?;
        let new_ref = self.ctx.repo.refs.current_ref(HEAD)?;

        let target = self.target.clone();
        self.print_previous_head(&current_ref, &current_oid, &target_oid)?;
        self.print_detachment_notice(&current_ref, &new_ref, &target)?;
        self.print_new_head(&current_ref, &new_ref, &target, &target_oid)?;

        Ok(())
    }

    fn print_previous_head(
        &mut self,
        current_ref: &Ref,
        current_oid: &str,
        target_oid: &str,
    ) -> Result<()> {
        if current_ref.is_head() && current_oid != target_oid {
            self.print_head_position("Previous HEAD position was", &current_oid)?;
        }

        Ok(())
    }

    fn print_detachment_notice(
        &self,
        current_ref: &Ref,
        new_ref: &Ref,
        target: &str,
    ) -> Result<()> {
        if new_ref.is_head() && !current_ref.is_head() {
            let mut stderr = self.ctx.stderr.borrow_mut();

            writeln!(stderr, "Note: checking out '{}'.", target)?;
            writeln!(stderr)?;
            writeln!(stderr, "{}", DETACHED_HEAD_MESSAGE)?;
        }

        Ok(())
    }

    fn print_new_head(
        &mut self,
        current_ref: &Ref,
        new_ref: &Ref,
        target: &str,
        target_oid: &str,
    ) -> Result<()> {
        if new_ref.is_head() {
            self.print_head_position("HEAD is now at", &target_oid)?;
        } else if new_ref == current_ref {
            let mut stderr = self.ctx.stderr.borrow_mut();
            writeln!(stderr, "Already on '{}'", target)?;
        } else {
            let mut stderr = self.ctx.stderr.borrow_mut();
            writeln!(stderr, "Switched to branch '{}'", target)?;
        }

        Ok(())
    }

    fn print_head_position(&mut self, message: &str, oid: &str) -> Result<()> {
        match self.ctx.repo.database.load(&oid)? {
            ParsedObject::Commit(commit) => {
                let short = Database::short_oid(&oid);

                let mut stderr = self.ctx.stderr.borrow_mut();
                writeln!(stderr, "{} {} {}", message, short, commit.title_line())?;
            }
            _ => unreachable!(),
        }

        Ok(())
    }
}
