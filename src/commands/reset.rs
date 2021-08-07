use crate::commands::{Command, CommandContext};
use crate::database::tree::TreeEntry;
use crate::errors::{Error, Result};
use crate::revision::{Revision, COMMIT};
use crate::util::path_to_string;
use std::path::{Path, PathBuf};

enum Mode {
    Soft,
    Mixed,
    Hard,
}

pub struct Reset<'a> {
    ctx: CommandContext<'a>,
    commit_oid: Option<String>,
    mode: Mode,
    /// `jit reset <paths>...`
    paths: Vec<PathBuf>,
}

impl<'a> Reset<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Result<Self> {
        let (paths, mode) = match &ctx.opt.cmd {
            Command::Reset {
                files,
                soft,
                _mixed,
                hard,
            } => {
                let mode = if *hard {
                    Mode::Hard
                } else if *soft {
                    Mode::Soft
                } else {
                    Mode::Mixed
                };
                (files.to_owned(), mode)
            }
            _ => unreachable!(),
        };

        let head_oid = ctx.repo.refs.read_head()?;

        Ok(Self {
            ctx,
            commit_oid: head_oid,
            mode,
            paths,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        self.select_commit_id()?;

        self.ctx.repo.index.load_for_update()?;
        self.reset_files()?;
        self.ctx.repo.index.write_updates()?;

        if let Some(commit_oid) = &self.commit_oid {
            if self.paths.is_empty() {
                self.ctx.repo.refs.update_head(commit_oid)?;
            }
        }

        Ok(())
    }

    fn select_commit_id(&mut self) -> Result<()> {
        if let Some(revision) = self.paths.get(0) {
            match Revision::new(&self.ctx.repo, &path_to_string(revision)).resolve(Some(COMMIT)) {
                Ok(commit_oid) => {
                    self.commit_oid = Some(commit_oid);
                    self.paths.remove(0);
                }
                Err(err) => match err {
                    Error::InvalidObject(..) => (),
                    _ => return Err(err),
                },
            }
        }

        Ok(())
    }

    fn reset_files(&mut self) -> Result<()> {
        if matches!(self.mode, Mode::Soft) {
            return Ok(());
        } else if matches!(self.mode, Mode::Hard) {
            self.ctx
                .repo
                .hard_reset(self.commit_oid.as_ref().unwrap())?;
            return Ok(());
        }

        if self.paths.is_empty() {
            self.ctx.repo.index.clear();
            self.reset_path(None)?;
        } else {
            let paths = self.paths.clone();
            for path in &paths {
                self.reset_path(Some(path))?;
            }
        }

        Ok(())
    }

    fn reset_path(&mut self, pathname: Option<&Path>) -> Result<()> {
        let listing = self
            .ctx
            .repo
            .database
            .load_tree_list(self.commit_oid.as_deref(), pathname)?;
        if let Some(pathname) = pathname {
            self.ctx.repo.index.remove(pathname);
        }

        for (path, entry) in listing {
            let entry = match entry {
                TreeEntry::Entry(entry) => entry,
                TreeEntry::Tree(_tree) => unreachable!(),
            };
            self.ctx.repo.index.add_from_db(&path, &entry);
        }

        Ok(())
    }
}
