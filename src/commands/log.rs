use crate::commands::shared::print_diff::PrintDiff;
use crate::commands::{Command, CommandContext};
use crate::database::commit::Commit;
use crate::database::object::Object;
use crate::database::{Database, ParsedObject};
use crate::errors::{Error, Result};
use crate::refs::Ref;
use crate::repository::Repository;
use crate::revision::{Revision, COMMIT, HEAD};
use colored::Colorize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write;
use structopt::clap::arg_enum;

arg_enum! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum LogFormat {
        Medium,
        OneLine,
    }
}

arg_enum! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum LogDecoration {
        Short,
        Full,
        Auto,
        No,
    }
}

pub struct Log<'a> {
    ctx: CommandContext<'a>,
    print_diff: PrintDiff,
    /// When false, calls to `Log.blank_line()` will not actually print a blank line.
    blank_line: RefCell<bool>,
    /// `jit log <commit>`
    args: Vec<String>,
    /// `jit log --abbrev-commit`
    abbrev: bool,
    /// `jit log --pretty=<format>` or `jit log --format=<format>`
    format: LogFormat,
    /// `jit log --patch`
    patch: bool,
    /// `jit log --decorate=<format>` or `jit log --no-decorate`
    decorate: LogDecoration,
    reverse_refs: Option<HashMap<String, Vec<Ref>>>,
    current_ref: Option<Ref>,
}

impl<'a> Log<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Self {
        let (args, abbrev, format, patch, decorate) = match &ctx.opt.cmd {
            Command::Log {
                args,
                abbrev,
                no_abbrev,
                format,
                one_line,
                decorate,
                no_decorate,
                patch,
                _no_patch,
            } => {
                let format = if *one_line {
                    LogFormat::OneLine
                } else {
                    format.to_owned()
                };

                // `--oneline --no-abbrev-commit` sets `abbrev = false`
                let abbrev = (*abbrev || *one_line) && !*no_abbrev;

                let decorate = if *no_decorate {
                    LogDecoration::No
                } else {
                    match decorate {
                        Some(None) => LogDecoration::Short,
                        Some(Some(decorate)) => decorate.to_owned(),
                        None => LogDecoration::Auto,
                    }
                };

                (args.to_owned(), abbrev, format, *patch, decorate)
            }
            _ => unreachable!(),
        };

        Self {
            ctx,
            print_diff: PrintDiff::new(),
            blank_line: RefCell::new(false),
            args,
            abbrev,
            format,
            patch,
            decorate,
            reverse_refs: None,
            current_ref: None,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        self.ctx.setup_pager();

        self.reverse_refs = Some(self.ctx.repo.refs.reverse_refs()?);
        self.current_ref = Some(self.ctx.repo.refs.current_ref("HEAD")?);

        for commit in Commits::new(
            &self.ctx.repo,
            self.args.get(0).unwrap_or(&String::from(HEAD)).to_string(),
        )? {
            let commit = commit?;
            self.show_commit(&commit)?;
        }

        Ok(())
    }

    fn show_commit(&self, commit: &Commit) -> Result<()> {
        match self.format {
            LogFormat::Medium => self.show_commit_medium(&commit)?,
            LogFormat::OneLine => self.show_commit_oneline(&commit)?,
        }

        self.show_patch(&commit)?;

        Ok(())
    }

    fn show_commit_medium(&self, commit: &Commit) -> Result<()> {
        let author = &commit.author;

        self.blank_line()?;
        let mut stdout = self.ctx.stdout.borrow_mut();
        writeln!(
            stdout,
            "{}{}",
            format!("commit {}", self.maybe_abbrev(&commit)).yellow(),
            self.decorate(&commit),
        )?;
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

    fn show_commit_oneline(&self, commit: &Commit) -> Result<()> {
        let mut stdout = self.ctx.stdout.borrow_mut();
        writeln!(
            stdout,
            "{}{} {}",
            self.maybe_abbrev(&commit).yellow(),
            self.decorate(&commit),
            commit.title_line(),
        )?;

        Ok(())
    }

    fn decorate(&self, commit: &Commit) -> String {
        if (self.decorate == LogDecoration::Auto && !self.ctx.isatty)
            || self.decorate == LogDecoration::No
        {
            return String::new();
        }

        let refs = self.reverse_refs.as_ref().unwrap().get(&commit.oid());
        if let Some(refs) = refs {
            let (head, refs): (Vec<_>, Vec<_>) = refs.iter().partition(|r#ref| {
                r#ref.is_head() && !self.current_ref.as_ref().unwrap().is_head()
            });
            let names: Vec<_> = refs
                .iter()
                .map(|r#ref| self.decoration_name(head.first(), r#ref))
                .collect();

            format!(
                " {}{}{}",
                "(".yellow(),
                names.join(&", ".yellow()),
                ")".yellow()
            )
        } else {
            String::new()
        }
    }

    fn decoration_name(&self, head: Option<&&Ref>, r#ref: &Ref) -> String {
        let mut name = match self.decorate {
            LogDecoration::Short | LogDecoration::Auto => self.ctx.repo.refs.short_name(&r#ref),
            LogDecoration::Full => match r#ref {
                Ref::SymRef { path } => path.to_owned(),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };
        name = name.bold().color(self.ref_color(&r#ref)).to_string();

        if let Some(head) = head {
            if r#ref == self.current_ref.as_ref().unwrap() {
                name = format!("{} {}", "HEAD ->".bold().color(self.ref_color(&head)), name);
            }
        }

        name
    }

    fn blank_line(&self) -> Result<()> {
        if self.format == LogFormat::OneLine {
            return Ok(());
        }

        let mut blank_line = self.blank_line.borrow_mut();
        if *blank_line {
            let mut stdout = self.ctx.stdout.borrow_mut();
            writeln!(stdout)?;
        }
        *blank_line = true;

        Ok(())
    }

    fn maybe_abbrev(&self, commit: &Commit) -> String {
        if self.abbrev {
            Database::short_oid(&commit.oid())
        } else {
            commit.oid()
        }
    }

    fn ref_color(&self, r#ref: &Ref) -> &str {
        if r#ref.is_head() {
            "cyan"
        } else {
            "green"
        }
    }

    fn show_patch(&self, commit: &Commit) -> Result<()> {
        if !self.patch {
            return Ok(());
        }

        self.blank_line()?;

        let mut stdout = self.ctx.stdout.borrow_mut();
        self.print_diff.print_commit_diff(
            &mut stdout,
            &self.ctx.repo,
            commit.parent.as_deref(),
            &commit.oid(),
        )?;

        Ok(())
    }
}

struct Commits<'a> {
    repo: &'a Repository,
    current_oid: Option<String>,
}

impl<'a> Commits<'a> {
    pub fn new(repo: &'a Repository, start: String) -> Result<Self> {
        let current_oid = Revision::new(&repo, &start).resolve(Some(COMMIT))?;

        Ok(Self {
            repo,
            current_oid: Some(current_oid),
        })
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
