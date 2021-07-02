use crate::commands::shared::diff_printer::DiffPrinter;
use crate::commands::{Command, CommandContext};
use crate::database::commit::Commit;
use crate::database::object::Object;
use crate::database::Database;
use crate::errors::Result;
use crate::refs::Ref;
use crate::rev_list::RevList;
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
    diff_printer: DiffPrinter,
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
            diff_printer: DiffPrinter::new(),
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

        // We need to pass rev_list down to `show_patch()`, but we can't pass the `RevList` we're
        // iterating over because iteration requires a mutable borrow. We work around this by
        // creating two identical `RevList`s and iterating over one and passing the other.
        // Inefficient? Yes, but I don't have any better ideas.
        let rev_list = RevList::new(&self.ctx.repo, &self.args)?;
        for commit in RevList::new(&self.ctx.repo, &self.args)? {
            self.show_commit(&commit, &rev_list)?;
        }

        Ok(())
    }

    fn show_commit(&self, commit: &Commit, rev_list: &RevList) -> Result<()> {
        match self.format {
            LogFormat::Medium => self.show_commit_medium(&commit)?,
            LogFormat::OneLine => self.show_commit_oneline(&commit)?,
        }

        self.show_patch(&commit, &rev_list)?;

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

    fn show_patch(&self, commit: &Commit, rev_list: &RevList) -> Result<()> {
        if !self.patch {
            return Ok(());
        }

        self.blank_line()?;

        let mut stdout = self.ctx.stdout.borrow_mut();
        self.diff_printer.print_commit_diff(
            &mut stdout,
            &self.ctx.repo,
            commit.parent().as_deref(),
            &commit.oid(),
            Some(rev_list),
        )?;

        Ok(())
    }
}
