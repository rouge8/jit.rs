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

        let tree_diff = self.ctx.repo.database.tree_diff(current_oid, target_oid)?;
        let mut migration = self.ctx.repo.migration(tree_diff);
        migration.apply_changes()?;

        Ok(())
    }
}