use crate::commands::{Command, CommandContext};
use crate::errors::{Error, Result};
use crate::util::path_to_string;
use std::path::{Path, PathBuf};

pub struct Rm<'a> {
    ctx: CommandContext<'a>,
    /// `jit rm <paths>...`
    paths: Vec<PathBuf>,
    /// `jit rm --cached`
    cached: bool,
    /// `jit rm -f`
    force: bool,
    head_oid: Option<String>,
    uncommitted: Vec<PathBuf>,
    unstaged: Vec<PathBuf>,
    both_changed: Vec<PathBuf>,
}

impl<'a> Rm<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Result<Self> {
        let (paths, cached, force) = match &ctx.opt.cmd {
            Command::Rm {
                files,
                cached,
                force,
            } => (files.to_owned(), *cached, *force),
            _ => unreachable!(),
        };

        let head_oid = ctx.repo.refs.read_head()?;

        Ok(Self {
            ctx,
            paths,
            cached,
            force,
            head_oid,
            uncommitted: Vec::new(),
            unstaged: Vec::new(),
            both_changed: Vec::new(),
        })
    }

    pub fn run(&mut self) -> Result<()> {
        self.ctx.repo.index.load_for_update()?;

        let paths = self.paths.clone();
        for path in &paths {
            match self.plan_removal(path) {
                Ok(()) => (),
                Err(err) => match err {
                    Error::RmUntrackedFile(..) => {
                        self.ctx.repo.index.release_lock()?;
                        let mut stderr = self.ctx.stderr.borrow_mut();
                        writeln!(stderr, "fatal: {}", err)?;

                        return Err(Error::Exit(128));
                    }
                    _ => return Err(err),
                },
            }
        }
        self.exit_on_errors()?;

        for path in &paths {
            self.remove_file(path)?;
        }
        self.ctx.repo.index.write_updates()?;

        Ok(())
    }

    fn plan_removal(&mut self, path: &Path) -> Result<()> {
        if !self.ctx.repo.index.tracked_file(path) {
            return Err(Error::RmUntrackedFile(path_to_string(path)));
        }

        if self.force {
            return Ok(());
        }

        let item = if let Some(head_oid) = &self.head_oid {
            self.ctx.repo.database.load_tree_entry(head_oid, path)?
        } else {
            None
        };
        let entry = self.ctx.repo.index.entry_for_path(&path_to_string(path), 0);
        let stat = self.ctx.repo.workspace.stat_file(path)?;

        let staged_change = self.ctx.repo.compare_tree_to_index(item.as_ref(), entry);
        let unstaged_change = if stat.is_some() {
            self.ctx
                .repo
                .compare_index_to_workspace(entry, stat.as_ref())?
        } else {
            None
        };

        if staged_change.is_some() && unstaged_change.is_some() {
            self.both_changed.push(path.to_path_buf());
        } else if staged_change.is_some() && !self.cached {
            self.uncommitted.push(path.to_path_buf());
        } else if unstaged_change.is_some() && !self.cached {
            self.unstaged.push(path.to_path_buf());
        }

        Ok(())
    }

    fn remove_file(&mut self, path: &Path) -> Result<()> {
        self.ctx.repo.index.remove(path);
        if !self.cached {
            self.ctx.repo.workspace.remove(path)?;
        }

        let mut stdout = self.ctx.stdout.borrow_mut();
        writeln!(stdout, "rm '{}'", path_to_string(path))?;

        Ok(())
    }

    fn exit_on_errors(&mut self) -> Result<()> {
        if self.both_changed.is_empty() && self.uncommitted.is_empty() && self.unstaged.is_empty() {
            return Ok(());
        }

        self.print_errors(
            &self.both_changed,
            "staged content different from both the file and the HEAD",
        )?;
        self.print_errors(&self.uncommitted, "changes staged in the index")?;
        self.print_errors(&self.unstaged, "local modifications")?;

        self.ctx.repo.index.release_lock()?;
        Err(Error::Exit(1))
    }

    fn print_errors(&self, paths: &[PathBuf], message: &str) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }

        let files_have = if paths.len() == 1 {
            "file has"
        } else {
            "files have"
        };

        let mut stderr = self.ctx.stderr.borrow_mut();
        writeln!(stderr, "error: the following {} {}:", files_have, message,)?;
        for path in paths {
            writeln!(stderr, "    {}", path_to_string(path))?;
        }

        Ok(())
    }
}
