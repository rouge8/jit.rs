use crate::commands::CommandContext;
use crate::errors::{Error, Result};
use std::io::Write;

pub struct Branch<E: Write> {
    ctx: CommandContext<E>,
}

impl<E: Write> Branch<E> {
    pub fn new(ctx: CommandContext<E>) -> Self {
        Self { ctx }
    }

    pub fn run(&self) -> Result<()> {
        self.create_branch()?;

        Ok(())
    }

    fn create_branch(&self) -> Result<()> {
        let branch_name = &self.ctx.argv[0];
        match self.ctx.repo.refs.create_branch(branch_name) {
            Ok(()) => Ok(()),
            Err(err) => match err {
                Error::InvalidBranch(..) => {
                    let mut stderr = self.ctx.stderr.borrow_mut();
                    writeln!(stderr, "fatal: {}", err)?;
                    Err(Error::Exit(128))
                }
                _ => Err(err),
            },
        }
    }
}
