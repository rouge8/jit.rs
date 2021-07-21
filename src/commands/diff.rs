use crate::commands::shared::diff_printer::{DiffPrinter, Target};
use crate::commands::{Command, CommandContext};
use crate::database::blob::Blob;
use crate::errors::Result;
use crate::index::Entry;
use crate::repository::ChangeType;
use crate::revision::Revision;
use itertools::Itertools;
use std::path::Path;

pub struct Diff<'a> {
    ctx: CommandContext<'a>,
    diff_printer: DiffPrinter,
    /// `jit diff <commit> <commit>`
    args: Vec<String>,
    /// `jit diff --cached` or `jit diff --staged`
    cached: bool,
    /// `jit diff --patch`
    patch: bool,
    /// `jit diff --base` or `jit diff --ours` or `jit diff --theirs`
    stage: u16,
}

impl<'a> Diff<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Self {
        let (args, cached, patch, stage) = match &ctx.opt.cmd {
            Command::Diff {
                args,
                cached,
                staged,
                patch,
                no_patch,
                stage,
            } => {
                let stage: u16 = if stage.base {
                    1
                } else if stage.ours {
                    2
                } else if stage.theirs {
                    3
                } else {
                    0
                };
                (
                    args.to_owned(),
                    *cached || *staged,
                    *patch || !*no_patch,
                    stage,
                )
            }
            _ => unreachable!(),
        };

        let diff_printer = DiffPrinter::new();

        Self {
            ctx,
            diff_printer,
            args,
            cached,
            patch,
            stage,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        self.ctx.repo.index.load()?;
        self.ctx.repo.initialize_status()?;

        self.ctx.setup_pager();

        if self.cached {
            self.diff_head_index()?;
        } else if self.args.len() == 2 {
            self.diff_commits()?;
        } else {
            self.diff_index_workspace()?;
        }

        Ok(())
    }

    fn diff_commits(&self) -> Result<()> {
        if !self.patch {
            return Ok(());
        }

        let mut args = vec![];
        for rev in &self.args {
            args.push(Revision::new(&self.ctx.repo, &rev).resolve(Some("commit"))?);
        }
        let mut stdout = self.ctx.stdout.borrow_mut();
        self.diff_printer.print_commit_diff(
            &mut stdout,
            &self.ctx.repo,
            Some(&args[0]),
            &args[1],
            None,
        )?;

        Ok(())
    }

    fn diff_head_index(&self) -> Result<()> {
        if !self.patch {
            return Ok(());
        }

        for path in self.ctx.repo.index_changes.keys() {
            let mut stdout = self.ctx.stdout.borrow_mut();
            let state = &self.ctx.repo.index_changes[path];
            match state {
                ChangeType::Added => {
                    let mut a = self.diff_printer.from_nothing(&path);
                    let mut b = self.from_index(&path)?;

                    self.diff_printer.print_diff(&mut stdout, &mut a, &mut b)?;
                }
                ChangeType::Modified => {
                    let mut a = self.from_head(&path)?;
                    let mut b = self.from_index(&path)?;

                    self.diff_printer.print_diff(&mut stdout, &mut a, &mut b)?;
                }
                ChangeType::Deleted => {
                    let mut a = self.from_head(&path)?;
                    let mut b = self.diff_printer.from_nothing(&path);

                    self.diff_printer.print_diff(&mut stdout, &mut a, &mut b)?;
                }
                ChangeType::Untracked => unreachable!(),
            }
        }

        Ok(())
    }

    fn diff_index_workspace(&self) -> Result<()> {
        if !self.patch {
            return Ok(());
        }

        let paths = self
            .ctx
            .repo
            .workspace_changes
            .keys()
            .into_iter()
            // Merge the two iterators in sorted order
            .merge(self.ctx.repo.conflicts.keys().into_iter());

        for path in paths {
            if self.ctx.repo.conflicts.contains_key(path) {
                self.print_conflict_diff(path)?;
            } else {
                self.print_workspace_diff(path)?;
            }
        }

        Ok(())
    }

    fn print_conflict_diff(&self, path: &str) -> Result<()> {
        let mut stdout = self.ctx.stdout.borrow_mut();
        writeln!(stdout, "* Unmerged path {}", path)?;

        if let Some(mut target) = self.from_index_stage(path, self.stage)? {
            self.diff_printer
                .print_diff(&mut stdout, &mut target, &mut self.from_file(path)?)?;
        }

        Ok(())
    }

    fn print_workspace_diff(&self, path: &str) -> Result<()> {
        let mut stdout = self.ctx.stdout.borrow_mut();
        let state = &self.ctx.repo.workspace_changes[path];
        match state {
            ChangeType::Modified => {
                let mut a = self.from_index(&path)?;
                let mut b = self.from_file(&path)?;

                self.diff_printer.print_diff(&mut stdout, &mut a, &mut b)?;
            }
            ChangeType::Deleted => {
                let mut a = self.from_index(&path)?;
                let mut b = self.diff_printer.from_nothing(&path);

                self.diff_printer.print_diff(&mut stdout, &mut a, &mut b)?;
            }
            _ => unreachable!(),
        }

        Ok(())
    }

    fn from_head(&self, path: &str) -> Result<Target> {
        let entry = &self.ctx.repo.head_tree[path];
        let oid = entry.oid();
        let blob = self.ctx.repo.database.load_blob(&oid)?;

        Ok(Target::new(
            path.to_string(),
            oid,
            Some(entry.mode()),
            blob.data,
        ))
    }

    fn from_index(&self, path: &str) -> Result<Target> {
        let entry = self.ctx.repo.index.entry_for_path(path, 0).unwrap();
        let blob = self.ctx.repo.database.load_blob(&entry.oid)?;

        Ok(Target::new(
            path.to_string(),
            entry.oid.clone(),
            Some(entry.mode),
            blob.data,
        ))
    }

    fn from_index_stage(&self, path: &str, stage: u16) -> Result<Option<Target>> {
        if let Some(entry) = self.ctx.repo.index.entry_for_path(path, stage) {
            let blob = self.ctx.repo.database.load_blob(&entry.oid)?;

            Ok(Some(Target::new(
                path.to_string(),
                entry.oid.clone(),
                Some(entry.mode),
                blob.data,
            )))
        } else {
            Ok(None)
        }
    }

    fn from_file(&self, path: &str) -> Result<Target> {
        let blob = Blob::new(self.ctx.repo.workspace.read_file(Path::new(path))?);
        let oid = self.ctx.repo.database.hash_object(&blob);
        let mode = Entry::mode_for_stat(&self.ctx.repo.stats[path]);

        Ok(Target::new(path.to_string(), oid, Some(mode), blob.data))
    }
}
