use crate::commands::{Command, CommandContext};
use crate::database::tree::TreeEntry;
use crate::errors::{Error, Result};
use crate::revision::{Revision, COMMIT};
use crate::util::path_to_string;
use std::path::{Path, PathBuf};

pub struct Reset<'a> {
    ctx: CommandContext<'a>,
    commit_oid: Option<String>,
    /// `jit reset <paths>...`
    paths: Vec<PathBuf>,
}

impl<'a> Reset<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Result<Self> {
        let paths = match &ctx.opt.cmd {
            Command::Reset { files } => files.to_owned(),
            _ => unreachable!(),
        };

        let head_oid = ctx.repo.refs.read_head()?;

        Ok(Self {
            ctx,
            commit_oid: head_oid,
            paths,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        self.select_commit_id()?;

        self.ctx.repo.index.load_for_update()?;
        let paths = self.paths.clone();
        for path in &paths {
            self.reset_path(path)?;
        }
        self.ctx.repo.index.write_updates()?;

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

    fn reset_path(&mut self, pathname: &Path) -> Result<()> {
        let listing = self
            .ctx
            .repo
            .database
            .load_tree_list(self.commit_oid.as_deref(), Some(pathname))?;
        self.ctx.repo.index.remove(pathname);

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
