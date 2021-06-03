use crate::commands::CommandContext;
use crate::errors::Result;
use crate::repository::Repository;
use crate::util::path_to_string;
use std::collections::BTreeSet;
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
            } else {
                let mut path = path_to_string(path);
                if stat.is_dir() {
                    path.push(MAIN_SEPARATOR);
                }
                untracked.insert(path);
            }
        }

        Ok(untracked)
    }
}