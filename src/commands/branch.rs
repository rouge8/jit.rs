use crate::commands::CommandContext;
use crate::errors::{Error, Result};
use crate::revision::Revision;
use std::io::Write;

pub struct Branch<E: Write> {
    ctx: CommandContext<E>,
}

impl<E: Write> Branch<E> {
    pub fn new(ctx: CommandContext<E>) -> Self {
        Self { ctx }
    }

    pub fn run(&mut self) -> Result<()> {
        self.create_branch()?;

        Ok(())
    }

    fn create_branch(&mut self) -> Result<()> {
        let branch_name = &self.ctx.argv[0];
        let start_point = self.ctx.argv.get(1);

        let start_oid = match start_point {
            Some(start_point) => {
                let mut revision = Revision::new(&mut self.ctx.repo, start_point);
                match revision.resolve() {
                    Ok(start_oid) => start_oid,
                    Err(err) => match err {
                        Error::InvalidObject(..) => {
                            let mut stderr = self.ctx.stderr.borrow_mut();

                            for error in revision.errors {
                                writeln!(stderr, "error: {}", error.message)?;
                                for line in error.hint {
                                    writeln!(stderr, "hint: {}", line)?;
                                }
                            }

                            writeln!(stderr, "fatal: {}", err)?;
                            return Err(Error::Exit(128));
                        }
                        _ => return Err(err),
                    },
                }
            }
            None => self.ctx.repo.refs.read_head()?.unwrap(),
        };

        match self.ctx.repo.refs.create_branch(branch_name, start_oid) {
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
