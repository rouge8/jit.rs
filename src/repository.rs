use crate::database::{blob::Blob, tree::TreeEntry, Database, ParsedObject};
use crate::errors::Result;
use crate::index::Entry;
use crate::index::Index;
use crate::refs::Refs;
use crate::util::path_to_string;
use crate::workspace::Workspace;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum ChangeType {
    Added,
    Deleted,
    Modified,
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
    stats: HashMap<String, fs::Metadata>,
    pub changed: BTreeSet<String>,
    pub index_changes: BTreeMap<String, ChangeType>,
    pub workspace_changes: BTreeMap<String, ChangeType>,
    pub untracked_files: BTreeSet<String>,
    head_tree: HashMap<String, TreeEntry>,
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

    fn trackable_file(&mut self, path: &Path, stat: &fs::Metadata) -> Result<bool> {
        if stat.is_file() {
            return Ok(!self.index.tracked(path));
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
            let commit = match self.database.load(head_oid)? {
                ParsedObject::Commit(commit) => commit,
                _ => unreachable!(),
            };
            let tree_oid = commit.tree.clone();
            self.read_tree(tree_oid, PathBuf::new())?;
        }

        Ok(())
    }

    fn read_tree(&mut self, tree_oid: String, pathname: PathBuf) -> Result<()> {
        let tree = match self.database.load(tree_oid)? {
            ParsedObject::Tree(tree) => tree,
            _ => unreachable!(),
        };

        let entries = tree.entries.clone();
        for (name, entry) in entries {
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
            self.check_index_against_workspace(&mut entry)?;
            self.check_index_against_head_tree(&entry);
        }

        // Update `self.index.entries` with the entries that were modified in
        // `self.check_index_entry()`
        for (key, val) in cloned_entries {
            self.index.entries.insert(key, val);
        }

        Ok(())
    }

    fn check_index_against_workspace(&mut self, entry: &mut Entry) -> Result<()> {
        let stat = match self.stats.get(&entry.path) {
            Some(stat) => stat,
            None => {
                self.record_change(&entry.path, ChangeKind::Workspace, ChangeType::Deleted);
                return Ok(());
            }
        };

        if !entry.stat_match(&stat) {
            self.record_change(&entry.path, ChangeKind::Workspace, ChangeType::Modified);
            return Ok(());
        }

        if entry.times_match(&stat) {
            return Ok(());
        }

        let data = self.workspace.read_file(&PathBuf::from(&entry.path))?;
        let blob = Blob::new(data);
        let oid = self.database.hash_object(&blob);

        if entry.oid == oid {
            self.index.update_entry_stat(entry, &stat);
        } else {
            self.record_change(&entry.path, ChangeKind::Workspace, ChangeType::Modified);
        }

        Ok(())
    }

    fn check_index_against_head_tree(&mut self, entry: &Entry) {
        match self.head_tree.get(&entry.path) {
            Some(item) => {
                if entry.mode != item.mode() || entry.oid != item.oid() {
                    self.record_change(&entry.path, ChangeKind::Index, ChangeType::Modified);
                }
            }
            None => self.record_change(&entry.path, ChangeKind::Index, ChangeType::Added),
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
}
