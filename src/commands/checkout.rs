use crate::commands::CommandContext;
use crate::errors::{Error, Result};
use crate::revision::{Revision, COMMIT};
use std::io::Write;

pub struct Checkout<E: Write> {
    ctx: CommandContext<E>,
}

impl<E: Write> Checkout<E> {
    pub fn new(ctx: CommandContext<E>) -> Self {
        Self { ctx }
    }

    pub fn run(&mut self) -> Result<()> {
        let target = &self.ctx.argv[0];

        let current_oid = self.ctx.repo.refs.read_head()?.unwrap();

        let mut revision = Revision::new(&mut self.ctx.repo, target);
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
        self.ctx.repo.refs.set_head(&target, &target_oid)?;

        Ok(())
    }
}
