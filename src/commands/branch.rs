use crate::commands::{Command, CommandContext};
use crate::database::object::Object;
use crate::database::{Database, ParsedObject};
use crate::errors::{Error, Result};
use crate::refs::{Ref, HEAD};
use crate::revision::{Revision, COMMIT};
use colored::Colorize;
use std::io::Write;

pub struct Branch<'a> {
    ctx: CommandContext<'a>,
    /// `jit branch <branch_name>`
    branch_name: Option<String>,
    /// `jit branch <branch_name> [start_point]`
    start_point: Option<String>,
    /// `jit branch --verbose`
    verbose: bool,
}

impl<'a> Branch<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Self {
        let (branch_name, start_point, verbose) = match &ctx.opt.cmd {
            Command::Branch {
                branch_name,
                start_point,
                verbose,
            } => (
                branch_name.to_owned(),
                start_point.to_owned(),
                verbose.to_owned(),
            ),
            _ => unreachable!(),
        };

        Self {
            ctx,
            branch_name,
            start_point,
            verbose,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        if self.branch_name.is_none() {
            self.list_branches()?;
        } else {
            self.create_branch()?;
        }

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
            .create_branch(self.branch_name.as_ref().unwrap(), start_oid)
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

    fn list_branches(&mut self) -> Result<()> {
        let current = self.ctx.repo.refs.current_ref(HEAD)?;
        let mut branches = self.ctx.repo.refs.list_branches()?;
        branches.sort_by_key(|branch| match branch {
            Ref::SymRef { path } => path.to_owned(),
            Ref::Ref { .. } => unreachable!(),
        });

        let max_width = branches
            .iter()
            .map(|branch| self.ctx.repo.refs.short_name(&branch).len())
            .max()
            .unwrap_or(0);

        self.ctx.setup_pager();

        for r#ref in branches {
            let info = self.format_ref(&r#ref, &current);
            let extended_info = self.extended_branch_info(&r#ref, max_width)?;

            let mut stdout = self.ctx.stdout.borrow_mut();
            writeln!(stdout, "{}{}", info, extended_info)?;
        }

        Ok(())
    }

    fn format_ref(&self, r#ref: &Ref, current: &Ref) -> String {
        let short_name = self.ctx.repo.refs.short_name(&r#ref);

        if r#ref == current {
            format!("* {}", short_name.green())
        } else {
            format!("  {}", short_name)
        }
    }

    fn extended_branch_info(&mut self, r#ref: &Ref, max_width: usize) -> Result<String> {
        if !self.verbose {
            return Ok(String::from(""));
        }

        let commit = match self
            .ctx
            .repo
            .database
            .load(&self.ctx.repo.refs.read_oid(&r#ref)?.unwrap())?
        {
            ParsedObject::Commit(commit) => commit,
            _ => unreachable!(),
        };
        let short = Database::short_oid(&commit.oid());
        let space = " ".repeat(max_width - self.ctx.repo.refs.short_name(&r#ref).len());

        Ok(format!("{} {} {}", space, short, commit.title_line()))
    }
}
