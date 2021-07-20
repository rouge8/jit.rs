use crate::database::{
    blob::Blob, tree::TreeEntry, tree_diff::TreeDiffChanges, Database, ParsedObject,
};
use crate::errors::Result;
use crate::index::{Entry as IndexEntry, Index};
use crate::refs::Refs;
use crate::util::path_to_string;
use crate::workspace::Workspace;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};

pub mod migration;

use migration::Migration;

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
    root_path: PathBuf,
    pub database: Database,
    pub index: Index,
    pub refs: Refs,
    pub workspace: Workspace,

    // status-related fields
    pub stats: HashMap<String, fs::Metadata>,
    pub changed: BTreeSet<String>,
    pub index_changes: BTreeMap<String, ChangeType>,
    pub conflicts: BTreeMap<String, Vec<u16>>,
    pub workspace_changes: BTreeMap<String, ChangeType>,
    pub untracked_files: BTreeSet<String>,
    pub head_tree: HashMap<String, TreeEntry>,
}

impl Repository {
    pub fn new(git_path: PathBuf) -> Self {
        let root_path = git_path.parent().unwrap().to_path_buf();

        Repository {
            root_path,
            database: Database::new(git_path.join("objects")),
            index: Index::new(git_path.join("index")),
            refs: Refs::new(git_path.clone()),
            workspace: Workspace::new(git_path.parent().unwrap().to_path_buf()),
            stats: HashMap::new(),
            changed: BTreeSet::new(),
            index_changes: BTreeMap::new(),
            conflicts: BTreeMap::new(),
            workspace_changes: BTreeMap::new(),
            untracked_files: BTreeSet::new(),
            head_tree: HashMap::new(),
        }
    }

    pub fn initialize_status(&mut self) -> Result<()> {
        self.scan_workspace(&self.root_path.clone())?;
        self.load_head_tree()?;
        self.check_index_entries()?;
        self.collect_deleted_head_files();

        Ok(())
    }

    pub fn migration(&mut self, tree_diff: TreeDiffChanges) -> Migration {
        Migration::new(self, tree_diff)
    }

    fn record_change(&mut self, path: &str, change_kind: ChangeKind, r#type: ChangeType) {
        self.changed.insert(path.to_string());

        let changes = match change_kind {
            ChangeKind::Index => &mut self.index_changes,
            ChangeKind::Workspace => &mut self.workspace_changes,
        };

        changes.insert(path.to_string(), r#type);
    }

    fn scan_workspace(&mut self, prefix: &Path) -> Result<()> {
        for (path, stat) in &self.workspace.list_dir(prefix)? {
            if self.index.tracked(path) {
                if stat.is_file() {
                    self.stats.insert(path_to_string(path), stat.clone());
                } else if stat.is_dir() {
                    self.scan_workspace(&path)?;
                }
            } else if self.trackable_file(&path, &stat)? {
                let mut path = path_to_string(path);
                if stat.is_dir() {
                    path.push(MAIN_SEPARATOR);
                }
                self.untracked_files.insert(path);
            }
        }

        Ok(())
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

    fn load_head_tree(&mut self) -> Result<()> {
        let head_oid = self.refs.read_head()?;

        if let Some(head_oid) = head_oid {
            let commit = self.database.load_commit(&head_oid)?;
            let tree_oid = commit.tree;
            self.read_tree(tree_oid, PathBuf::new())?;
        }

        Ok(())
    }

    fn read_tree(&mut self, tree_oid: String, pathname: PathBuf) -> Result<()> {
        let tree = match self.database.load(&tree_oid)? {
            ParsedObject::Tree(tree) => tree,
            _ => unreachable!(),
        };

        for (name, entry) in tree.entries {
            let path = pathname.join(name);

            if entry.is_tree() {
                self.read_tree(entry.oid(), path)?;
            } else {
                self.head_tree.insert(path_to_string(&path), entry);
            }
        }

        Ok(())
    }

    fn check_index_entries(&mut self) -> Result<()> {
        // We have to iterate over `cloned_entries` rather than `self.index.entries` because
        // Rust will not let us borrow self as mutable more than one time: first with
        // `self.index.entries.values_mut()` and second with `self.check_index_entry()`.
        let mut cloned_entries = self.index.entries.clone();
        for mut entry in cloned_entries.values_mut() {
            if entry.stage() == 0 {
                self.check_index_against_workspace(&mut entry)?;
                self.check_index_against_head_tree(&entry);
            } else {
                self.changed.insert(entry.path.clone());
                self.conflicts
                    .entry(entry.path.clone())
                    .or_insert_with(Vec::new)
                    .push(entry.stage());
            }
        }

        // Update `self.index.entries` with the entries that were modified in
        // `self.check_index_entry()`
        for (key, val) in cloned_entries {
            self.index.entries.insert(key, val);
        }

        Ok(())
    }

    fn check_index_against_workspace(&mut self, entry: &mut IndexEntry) -> Result<()> {
        let stat = self.stats.get(&entry.path);
        let status = self.compare_index_to_workspace(Some(&entry), stat)?;

        match status {
            Some(status) => self.record_change(&entry.path, ChangeKind::Workspace, status),
            None => self.index.update_entry_stat(entry, &stat.unwrap()),
        }

        Ok(())
    }

    fn check_index_against_head_tree(&mut self, entry: &IndexEntry) {
        let item = self.head_tree.get(&entry.path);
        let status = self.compare_tree_to_index(item, Some(&entry));

        if let Some(status) = status {
            self.record_change(&entry.path, ChangeKind::Index, status)
        }
    }

    fn collect_deleted_head_files(&mut self) {
        let keys: Vec<_> = self.head_tree.keys().cloned().collect();
        for path in keys {
            if !self.index.tracked_file(Path::new(&path)) {
                self.record_change(&path, ChangeKind::Index, ChangeType::Deleted);
            }
        }
    }

    fn compare_index_to_workspace(
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

        if !entry.stat_match(&stat) {
            return Ok(Some(ChangeType::Modified));
        } else if entry.times_match(&stat) {
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

    fn compare_tree_to_index(
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
