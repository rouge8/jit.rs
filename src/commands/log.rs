use crate::commands::CommandContext;
use crate::database::commit::Commit;
use crate::database::object::Object;
use crate::database::ParsedObject;
use crate::errors::{Error, Result};
use crate::repository::Repository;
use colored::Colorize;
use std::cell::RefCell;
use std::io::Write;

pub struct Log<'a> {
    ctx: CommandContext<'a>,
    /// When false, calls to `Log.blank_line()` will not actually print a blank line.
    blank_line: RefCell<bool>,
}

impl<'a> Log<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Self {
        Self {
            ctx,
            blank_line: RefCell::new(false),
        }
    }

    pub fn run(&mut self) -> Result<()> {
        self.ctx.setup_pager();

        for commit in Commits::new(&self.ctx.repo)? {
            let commit = commit?;
            self.show_commit(&commit)?;
        }

        Ok(())
    }

    fn show_commit(&self, commit: &Commit) -> Result<()> {
        let author = &commit.author;

        self.blank_line()?;
        let mut stdout = self.ctx.stdout.borrow_mut();
        writeln!(stdout, "{}", format!("commit {}", commit.oid()).yellow())?;
        writeln!(stdout, "Author: {} <{}>", author.name, author.email)?;
        writeln!(stdout, "Date:   {}", author.readable_time())?;
        drop(stdout);
        self.blank_line()?;

        let mut stdout = self.ctx.stdout.borrow_mut();
        for line in commit.message.lines() {
            writeln!(stdout, "    {}", line)?;
        }

        Ok(())
    }

    fn blank_line(&self) -> Result<()> {
        let mut blank_line = self.blank_line.borrow_mut();

        if *blank_line {
            let mut stdout = self.ctx.stdout.borrow_mut();
            writeln!(stdout)?;
        }
        *blank_line = true;

        Ok(())
    }
}

struct Commits<'a> {
    repo: &'a Repository,
    current_oid: Option<String>,
}

impl<'a> Commits<'a> {
    pub fn new(repo: &'a Repository) -> Result<Self> {
        let current_oid = repo.refs.read_head()?;

        Ok(Self { repo, current_oid })
    }
}

impl<'a> Iterator for Commits<'a> {
    type Item = Result<Commit>;

    fn next(&mut self) -> Option<Self::Item> {
        self.current_oid.as_ref()?;

        match self.repo.database.load(&self.current_oid.as_ref().unwrap()) {
            Ok(ParsedObject::Commit(commit)) => {
                self.current_oid = commit.parent.clone();

                Some(Ok(commit))
            }
            Err(err) => Some(Err(Error::Io(err))),
            _ => unreachable!(),
        }
    }
}
