use crate::commands::{Command, CommandContext};
use crate::database::tree::TreeEntry;
use crate::errors::Result;
use std::path::{Path, PathBuf};

pub struct Reset<'a> {
    ctx: CommandContext<'a>,
    head_oid: Option<String>,
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
            head_oid,
            paths,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        self.ctx.repo.index.load_for_update()?;
        let paths = self.paths.clone();
        for path in &paths {
            self.reset_path(path)?;
        }
        self.ctx.repo.index.write_updates()?;

        Ok(())
    }

    fn reset_path(&mut self, pathname: &Path) -> Result<()> {
        let listing = self
            .ctx
            .repo
            .database
            .load_tree_list(self.head_oid.as_deref(), Some(pathname))?;
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
