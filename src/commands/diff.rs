use crate::commands::shared::print_diff::{PrintDiff, Target};
use crate::commands::{Command, CommandContext};
use crate::database::blob::Blob;
use crate::database::ParsedObject;
use crate::errors::Result;
use crate::index::Entry;
use crate::repository::ChangeType;
use std::path::Path;

pub struct Diff<'a> {
    ctx: CommandContext<'a>,
    print_diff: PrintDiff,
    /// `jit diff --cached` or `jit diff --staged`
    cached: bool,
    /// `jit diff --patch`
    patch: bool,
}

impl<'a> Diff<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Self {
        let (cached, patch) = match ctx.opt.cmd {
            Command::Diff {
                cached,
                staged,
                patch,
                no_patch,
            } => (cached || staged, patch || !no_patch),
            _ => unreachable!(),
        };

        let print_diff = PrintDiff::new();

        Self {
            ctx,
            print_diff,
            cached,
            patch,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        self.ctx.repo.index.load()?;
        self.ctx.repo.initialize_status()?;

        self.ctx.setup_pager();

        if self.cached {
            self.diff_head_index()?;
        } else {
            self.diff_index_workspace()?;
        }

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
                    let mut a = self.print_diff.from_nothing(&path);
                    let mut b = self.from_index(&path)?;

                    self.print_diff.print_diff(&mut stdout, &mut a, &mut b)?;
                }
                ChangeType::Modified => {
                    let mut a = self.from_head(&path)?;
                    let mut b = self.from_index(&path)?;

                    self.print_diff.print_diff(&mut stdout, &mut a, &mut b)?;
                }
                ChangeType::Deleted => {
                    let mut a = self.from_head(&path)?;
                    let mut b = self.print_diff.from_nothing(&path);

                    self.print_diff.print_diff(&mut stdout, &mut a, &mut b)?;
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

        for path in self.ctx.repo.workspace_changes.keys() {
            let mut stdout = self.ctx.stdout.borrow_mut();
            let state = &self.ctx.repo.workspace_changes[path];
            match state {
                ChangeType::Modified => {
                    let mut a = self.from_index(&path)?;
                    let mut b = self.from_file(&path)?;

                    self.print_diff.print_diff(&mut stdout, &mut a, &mut b)?;
                }
                ChangeType::Deleted => {
                    let mut a = self.from_index(&path)?;
                    let mut b = self.print_diff.from_nothing(&path);

                    self.print_diff.print_diff(&mut stdout, &mut a, &mut b)?;
                }
                _ => unreachable!(),
            }
        }

        Ok(())
    }

    fn from_head(&self, path: &str) -> Result<Target> {
        let entry = &self.ctx.repo.head_tree[path];
        let oid = entry.oid();
        let blob = match self.ctx.repo.database.load(&oid)? {
            ParsedObject::Blob(blob) => blob,
            _ => unreachable!(),
        };

        Ok(Target::new(
            path.to_string(),
            oid,
            Some(entry.mode()),
            blob.data,
        ))
    }

    fn from_index(&self, path: &str) -> Result<Target> {
        let entry = self.ctx.repo.index.entry_for_path(path).unwrap();
        let blob = match self.ctx.repo.database.load(&entry.oid)? {
            ParsedObject::Blob(blob) => blob,
            _ => unreachable!(),
        };

        Ok(Target::new(
            path.to_string(),
            entry.oid.clone(),
            Some(entry.mode),
            blob.data,
        ))
    }

    fn from_file(&self, path: &str) -> Result<Target> {
        let blob = Blob::new(self.ctx.repo.workspace.read_file(Path::new(path))?);
        let oid = self.ctx.repo.database.hash_object(&blob);
        let mode = Entry::mode_for_stat(&self.ctx.repo.stats[path]);

        Ok(Target::new(path.to_string(), oid, Some(mode), blob.data))
    }
}
