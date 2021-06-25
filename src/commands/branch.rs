use crate::commands::{Command, CommandContext};
use crate::errors::{Error, Result};
use crate::revision::{Revision, COMMIT};
use std::io::Write;

pub struct Branch<'a> {
    ctx: CommandContext<'a>,
    /// `jit branch <branch_name>`
    branch_name: String,
    /// `jit branch <branch_name> [start_point]`
    start_point: Option<String>,
}

impl<'a> Branch<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Self {
        let (branch_name, start_point) = match &ctx.opt.cmd {
            Command::Branch {
                branch_name,
                start_point,
            } => (branch_name.to_owned(), start_point.to_owned()),
            _ => unreachable!(),
        };

        Self {
            ctx,
            branch_name,
            start_point,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        self.create_branch()?;

        Ok(())
    }

    fn create_branch(&mut self) -> Result<()> {
        let start_oid = match &self.start_point {
            Some(start_point) => {
                let mut revision = Revision::new(&mut self.ctx.repo, &start_point);
                match revision.resolve(Some(COMMIT)) {
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

        match self
            .ctx
            .repo
            .refs
            .create_branch(&self.branch_name, start_oid)
        {
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
