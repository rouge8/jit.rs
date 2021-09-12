use crate::commands::{Command, CommandContext};
use crate::errors::{Error, Result};

pub struct Remote<'a> {
    ctx: CommandContext<'a>,
    args: Vec<String>,
    tracked: Vec<String>,
    verbose: bool,
}

impl<'a> Remote<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Self {
        let (args, tracked, verbose) = match &ctx.opt.cmd {
            Command::Remote {
                args,
                verbose,
                tracked,
            } => (args.to_owned(), tracked.to_owned(), *verbose),
            _ => unreachable!(),
        };
        Self {
            ctx,
            args,
            tracked,
            verbose,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        if self.args.is_empty() {
            self.list_remotes()?;
        } else {
            match self.args.remove(0).as_str() {
                "add" => self.add_remote()?,
                "remove" => self.remove_remote()?,
                _ => unimplemented!(),
            }
        }

        Ok(())
    }

    fn add_remote(&mut self) -> Result<()> {
        let (name, url) = (&self.args[0], &self.args[1]);

        match self.ctx.repo.remotes.add(name, url, &self.tracked) {
            Ok(()) => Err(Error::Exit(0)),
            Err(err) => match err {
                Error::InvalidRemote(..) => {
                    let mut stderr = self.ctx.stderr.borrow_mut();
                    writeln!(stderr, "fatal: {}", err)?;
                    Err(Error::Exit(128))
                }
                _ => Err(err),
            },
        }
    }

    fn remove_remote(&mut self) -> Result<()> {
        match self.ctx.repo.remotes.remove(&self.args[0]) {
            Ok(()) => Err(Error::Exit(0)),
            Err(err) => match err {
                Error::InvalidRemote(..) => {
                    let mut stderr = self.ctx.stderr.borrow_mut();
                    writeln!(stderr, "fatal: {}", err)?;
                    Err(Error::Exit(128))
                }
                _ => Err(err),
            },
        }
    }

    fn list_remotes(&self) -> Result<()> {
        for name in self.ctx.repo.remotes.list_remotes()? {
            self.list_remote(&name)?;
        }

        Err(Error::Exit(0))
    }

    fn list_remote(&self, name: &str) -> Result<()> {
        let mut stdout = self.ctx.stdout.borrow_mut();

        if !self.verbose {
            writeln!(stdout, "{}", name)?;
        } else {
            let remote = self.ctx.repo.remotes.get(name)?.unwrap();

            writeln!(stdout, "{}\t{} (fetch)", name, remote.fetch_url().unwrap())?;
            writeln!(stdout, "{}\t{} (push)", name, remote.push_url().unwrap())?;
        }

        Ok(())
    }
}
