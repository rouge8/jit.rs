use crate::commands::CommandContext;
use crate::errors::Result;
use crate::repository::Repository;
use crate::util::path_to_string;
use std::collections::BTreeSet;
use std::fs::Metadata;
use std::io::Read;
use std::path::{Path, MAIN_SEPARATOR};

pub struct Status;

impl Status {
    pub fn run<I: Read>(mut ctx: CommandContext<I>) -> Result<()> {
        ctx.repo.index.load()?;

        let untracked = Self::scan_workplace(&ctx.repo, &ctx.dir)?;

        for path in untracked {
            println!("?? {}", path);
        }

        Ok(())
    }

    fn scan_workplace(repo: &Repository, prefix: &Path) -> Result<BTreeSet<String>> {
        let mut untracked: BTreeSet<String> = BTreeSet::new();

        for (path, stat) in &repo.workspace.list_dir(prefix)? {
            if repo.index.tracked(path) {
                if stat.is_dir() {
                    untracked.append(&mut Self::scan_workplace(repo, &path)?);
                }
            } else if Self::trackable_file(repo, &path, &stat)? {
                let mut path = path_to_string(path);
                if stat.is_dir() {
                    path.push(MAIN_SEPARATOR);
                }
                untracked.insert(path);
            }
        }

        Ok(untracked)
    }

    fn trackable_file(repo: &Repository, path: &Path, stat: &Metadata) -> Result<bool> {
        if stat.is_file() {
            return Ok(!repo.index.tracked(path));
        } else if !stat.is_dir() {
            return Ok(false);
        }

        let items = repo.workspace.list_dir(path)?;
        let files = items.iter().filter(|(_, item_stat)| item_stat.is_file());
        let dirs = items.iter().filter(|(_, item_stat)| item_stat.is_dir());

        for (item_path, item_stat) in files.chain(dirs) {
            if Self::trackable_file(repo, item_path, item_stat)? {
                return Ok(true);
            }
        }

        Ok(false)
    }
}
