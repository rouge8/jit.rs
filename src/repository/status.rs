use crate::database::tree::TreeEntry;
use crate::errors::Result;
use crate::index::Entry as IndexEntry;
use crate::repository::{ChangeKind, ChangeType, Repository};
use crate::util::path_to_string;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, MAIN_SEPARATOR};

#[derive(Debug)]
pub struct Status {
    repo: *mut Repository,
    commit_oid: Option<String>,
    pub stats: HashMap<String, fs::Metadata>,
    pub changed: BTreeSet<String>,
    pub index_changes: BTreeMap<String, ChangeType>,
    pub conflicts: BTreeMap<String, Vec<u16>>,
    pub workspace_changes: BTreeMap<String, ChangeType>,
    pub untracked_files: BTreeSet<String>,
    pub head_tree: HashMap<String, TreeEntry>,
}

impl Status {
    /// You **must** call `status.initialize()` after `repo.index.load()` or
    /// `repo.index.load_for_update()`.
    pub fn new(repo: &mut Repository, commit_oid: Option<&str>) -> Self {
        Self {
            repo,
            commit_oid: commit_oid.map(|oid| oid.to_owned()),
            stats: HashMap::new(),
            changed: BTreeSet::new(),
            index_changes: BTreeMap::new(),
            conflicts: BTreeMap::new(),
            workspace_changes: BTreeMap::new(),
            untracked_files: BTreeSet::new(),
            head_tree: HashMap::new(),
        }
    }

    /// Call after `repo.index.load()` or `repo.index.load_for_update()`.
    pub fn initialize(&mut self) -> Result<()> {
        let commit_oid = if self.commit_oid.is_some() {
            self.commit_oid.clone()
        } else {
            unsafe { (*self.repo).refs.read_head()? }
        };

        unsafe {
            self.head_tree = (*self.repo)
                .database
                .load_tree_list(commit_oid.as_deref(), None)?;

            self.scan_workspace(&(*self.repo).root_path)?;
        }
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
        unsafe {
            for (path, stat) in &(*self.repo).workspace.list_dir(prefix)? {
                if (*self.repo).index.tracked(path) {
                    if stat.is_file() {
                        self.stats.insert(path_to_string(path), stat.clone());
                    } else if stat.is_dir() {
                        self.scan_workspace(path)?;
                    }
                } else if (*self.repo).trackable_file(path, stat)? {
                    let mut path = path_to_string(path);
                    if stat.is_dir() {
                        path.push(MAIN_SEPARATOR);
                    }
                    self.untracked_files.insert(path);
                }
            }
        }

        Ok(())
    }

    fn check_index_entries(&mut self) -> Result<()> {
        unsafe {
            for entry in (*self.repo).index.entries.values_mut() {
                if entry.stage() == 0 {
                    self.check_index_against_workspace(entry)?;
                    self.check_index_against_head_tree(entry);
                } else {
                    self.changed.insert(entry.path.clone());
                    self.conflicts
                        .entry(entry.path.clone())
                        .or_insert_with(Vec::new)
                        .push(entry.stage());
                }
            }
        }

        Ok(())
    }

    fn check_index_against_workspace(&mut self, entry: &mut IndexEntry) -> Result<()> {
        let stat = self.stats.get(&entry.path);
        unsafe {
            let status = (*self.repo).compare_index_to_workspace(Some(entry), stat)?;

            match status {
                Some(status) => self.record_change(&entry.path, ChangeKind::Workspace, status),
                None => (*self.repo).index.update_entry_stat(entry, stat.unwrap()),
            }
        }

        Ok(())
    }

    fn check_index_against_head_tree(&mut self, entry: &IndexEntry) {
        let item = self.head_tree.get(&entry.path);
        unsafe {
            let status = (*self.repo).compare_tree_to_index(item, Some(entry));

            if let Some(status) = status {
                self.record_change(&entry.path, ChangeKind::Index, status)
            }
        }
    }

    fn collect_deleted_head_files(&mut self) {
        let keys: Vec<_> = self.head_tree.keys().cloned().collect();
        for path in keys {
            unsafe {
                if !(*self.repo).index.tracked_file(Path::new(&path)) {
                    self.record_change(&path, ChangeKind::Index, ChangeType::Deleted);
                }
            }
        }
    }
}
