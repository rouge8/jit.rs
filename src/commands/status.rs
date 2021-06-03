use crate::commands::CommandContext;
use crate::errors::Result;
use crate::repository::Repository;
use crate::util::path_to_string;
use std::collections::BTreeSet;
use std::fs::Metadata;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};

pub struct Status {
    root_dir: PathBuf,
    repo: Repository,
}

impl Status {
    pub fn new(ctx: CommandContext) -> Self {
        Self {
            root_dir: ctx.dir,
            repo: ctx.repo,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        self.repo.index.load()?;

        let untracked = self.scan_workplace(&self.root_dir.clone())?;

        for path in untracked {
            println!("?? {}", path);
        }

        Ok(())
    }

    fn scan_workplace(&mut self, prefix: &Path) -> Result<BTreeSet<String>> {
        let mut untracked: BTreeSet<String> = BTreeSet::new();

        for (path, stat) in &self.repo.workspace.list_dir(prefix)? {
            if self.repo.index.tracked(path) {
                if stat.is_dir() {
                    untracked.append(&mut self.scan_workplace(&path)?);
                }
            } else if self.trackable_file(&path, &stat)? {
                let mut path = path_to_string(path);
                if stat.is_dir() {
                    path.push(MAIN_SEPARATOR);
                }
                untracked.insert(path);
            }
        }

        Ok(untracked)
    }

    fn trackable_file(&mut self, path: &Path, stat: &Metadata) -> Result<bool> {
        if stat.is_file() {
            return Ok(!self.repo.index.tracked(path));
        } else if !stat.is_dir() {
            return Ok(false);
        }

        let items = self.repo.workspace.list_dir(path)?;
        let files = items.iter().filter(|(_, item_stat)| item_stat.is_file());
        let dirs = items.iter().filter(|(_, item_stat)| item_stat.is_dir());

        for (item_path, item_stat) in files.chain(dirs) {
            if self.trackable_file(item_path, item_stat)? {
                return Ok(true);
            }
        }

        Ok(false)
    }
}
