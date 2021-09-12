use crate::config::stack::{ConfigFile, Stack as ConfigStack};
use crate::database::{blob::Blob, tree::TreeEntry, tree_diff::TreeDiffChanges, Database};
use crate::errors::Result;
use crate::index::{Entry as IndexEntry, Index};
use crate::refs::Refs;
use crate::remotes::Remotes;
use crate::repository::pending_commit::PendingCommit;
use crate::workspace::Workspace;
use std::fs;
use std::path::{Path, PathBuf};

mod hard_reset;
pub mod migration;
pub mod pending_commit;
pub mod sequencer;
pub mod status;

use hard_reset::HardReset;
use migration::Migration;
use status::Status;

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum ChangeType {
    Added,
    Deleted,
    Modified,
    Untracked,
}

#[derive(Debug)]
enum ChangeKind {
    Workspace,
    Index,
}

#[derive(Debug)]
pub struct Repository {
    pub root_path: PathBuf,
    pub git_path: PathBuf,
    pub database: Database,
    pub index: Index,
    pub refs: Refs,
    pub workspace: Workspace,
    pub config: ConfigStack,
    pub remotes: Remotes,
}

impl Repository {
    pub fn new(git_path: PathBuf) -> Self {
        let root_path = git_path.parent().unwrap().to_path_buf();
        let mut config = ConfigStack::new(&git_path);
        let remotes = Remotes::new(config.file(ConfigFile::Local));

        Repository {
            root_path,
            git_path: git_path.clone(),
            database: Database::new(git_path.join("objects")),
            index: Index::new(git_path.join("index")),
            refs: Refs::new(git_path.clone()),
            workspace: Workspace::new(git_path.parent().unwrap().to_path_buf()),
            config,
            remotes,
        }
    }

    pub fn hard_reset(&mut self, oid: &str) -> Result<()> {
        HardReset::new(self, oid).execute()?;

        Ok(())
    }

    pub fn migration(&mut self, tree_diff: TreeDiffChanges) -> Migration {
        Migration::new(self, tree_diff)
    }

    pub fn pending_commit(&self) -> PendingCommit {
        PendingCommit::new(&self.git_path)
    }

    pub fn status(&mut self, commit_oid: Option<&str>) -> Status {
        Status::new(self, commit_oid)
    }

    fn trackable_file(&self, path: &Path, stat: &fs::Metadata) -> Result<bool> {
        if stat.is_file() {
            return Ok(!self.index.tracked_file(path));
        } else if !stat.is_dir() {
            return Ok(false);
        }

        let items = self.workspace.list_dir(path)?;
        let files = items.iter().filter(|(_, item_stat)| item_stat.is_file());
        let dirs = items.iter().filter(|(_, item_stat)| item_stat.is_dir());

        for (item_path, item_stat) in files.chain(dirs) {
            if self.trackable_file(item_path, item_stat)? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn compare_index_to_workspace(
        &self,
        entry: Option<&IndexEntry>,
        stat: Option<&fs::Metadata>,
    ) -> Result<Option<ChangeType>> {
        if entry.is_none() {
            return Ok(Some(ChangeType::Untracked));
        } else if stat.is_none() {
            return Ok(Some(ChangeType::Deleted));
        }

        let entry = entry.unwrap();
        let stat = stat.unwrap();

        if !entry.stat_match(stat) {
            return Ok(Some(ChangeType::Modified));
        } else if entry.times_match(stat) {
            return Ok(None);
        }

        let data = self.workspace.read_file(Path::new(&entry.path))?;
        let blob = Blob::new(data);
        let oid = self.database.hash_object(&blob);

        if entry.oid != oid {
            Ok(Some(ChangeType::Modified))
        } else {
            Ok(None)
        }
    }

    pub fn compare_tree_to_index(
        &self,
        item: Option<&TreeEntry>,
        entry: Option<&IndexEntry>,
    ) -> Option<ChangeType> {
        if item.is_none() && entry.is_none() {
            return None;
        } else if item.is_none() {
            return Some(ChangeType::Added);
        } else if entry.is_none() {
            return Some(ChangeType::Deleted);
        }

        let item = item.unwrap();
        let entry = entry.unwrap();

        if !(entry.mode == item.mode() && entry.oid == item.oid()) {
            Some(ChangeType::Modified)
        } else {
            None
        }
    }
}
