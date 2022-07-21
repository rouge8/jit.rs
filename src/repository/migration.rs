use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;

use crate::database::entry::Entry;
use crate::database::tree::TreeEntry;
use crate::database::tree_diff::TreeDiffChanges;
use crate::errors::{Error, Result};
use crate::index::Entry as IndexEntry;
use crate::repository::Repository;
use crate::util::{parent_directories, path_to_string};

static MESSAGES: Lazy<HashMap<ConflictType, (&'static str, &'static str)>> = Lazy::new(|| {
    HashMap::from([
        (
            ConflictType::StaleFile,
            (
                "Your local changes to the following files would be overwritten by checkout:",
                "Please commit your changes or stash them before you switch branches.",
            ),
        ),
        (
            ConflictType::StaleDirectory,
            (
                "Updating the following directories would lose untracked files in them:",
                "",
            ),
        ),
        (
            ConflictType::UntrackedOverwritten,
            (
                "The following untracked working tree files would be overwritten by checkout:",
                "Please move or remove them before you switch branches.",
            ),
        ),
        (
            ConflictType::UntrackedRemoved,
            (
                "The following untracked working tree files would be removed by checkout:",
                "Please move or remove them before you switch branches.",
            ),
        ),
    ])
});

pub struct Migration<'a> {
    repo: &'a mut Repository,
    diff: TreeDiffChanges,
    pub changes: HashMap<Action, Vec<(PathBuf, Option<Entry>)>>,
    pub mkdirs: BTreeSet<PathBuf>,
    pub rmdirs: BTreeSet<PathBuf>,
    pub errors: Vec<String>,
    pub conflicts: HashMap<ConflictType, BTreeSet<PathBuf>>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Action {
    Create,
    Delete,
    Update,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum ConflictType {
    StaleFile,
    StaleDirectory,
    UntrackedOverwritten,
    UntrackedRemoved,
}

impl<'a> Migration<'a> {
    pub fn new(repo: &'a mut Repository, diff: TreeDiffChanges) -> Self {
        let changes = HashMap::from([
            (Action::Create, vec![]),
            (Action::Delete, vec![]),
            (Action::Update, vec![]),
        ]);

        let conflicts = HashMap::from([
            (ConflictType::StaleFile, BTreeSet::new()),
            (ConflictType::StaleDirectory, BTreeSet::new()),
            (ConflictType::UntrackedOverwritten, BTreeSet::new()),
            (ConflictType::UntrackedRemoved, BTreeSet::new()),
        ]);

        Self {
            repo,
            diff,
            changes,
            mkdirs: BTreeSet::new(),
            rmdirs: BTreeSet::new(),
            errors: Vec::new(),
            conflicts,
        }
    }

    pub fn apply_changes(&mut self) -> Result<()> {
        self.plan_changes()?;
        self.update_workspace()?;
        self.update_index()?;

        Ok(())
    }

    pub fn blob_data(&self, oid: &str) -> Result<Vec<u8>> {
        Ok(self.repo.database.load_blob(oid)?.data)
    }

    fn plan_changes(&mut self) -> Result<()> {
        // TODO: Pass `diff` as an argument to `apply_changes()` instead of cloning?
        for (path, (old_item, new_item)) in &self.diff.clone() {
            self.check_for_conflict(path, old_item, new_item)?;
            self.record_change(path, old_item, new_item);
        }

        self.collect_errors()?;

        Ok(())
    }

    fn record_change(&mut self, path: &Path, old_item: &Option<Entry>, new_item: &Option<Entry>) {
        let ancestors = path
            .ancestors()
            .map(|path| path.to_path_buf())
            .filter(|path| path.parent().is_some());

        let action = if old_item.is_none() {
            for path in ancestors {
                self.mkdirs.insert(path);
            }
            Action::Create
        } else if new_item.is_none() {
            for path in ancestors {
                self.rmdirs.insert(path);
            }
            Action::Delete
        } else {
            for path in ancestors {
                self.mkdirs.insert(path);
            }
            Action::Update
        };

        self.changes
            .get_mut(&action)
            .unwrap()
            .push((path.to_path_buf(), new_item.to_owned()));
    }

    fn update_workspace(&self) -> Result<()> {
        self.repo.workspace.apply_migration(self)?;
        Ok(())
    }

    fn update_index(&mut self) -> Result<()> {
        for (path, _) in &self.changes[&Action::Delete] {
            self.repo.index.remove(path);
        }

        for action in [Action::Create, Action::Update] {
            for (path, entry) in &self.changes[&action] {
                let stat = self.repo.workspace.stat_file(path)?.unwrap();
                self.repo.index.add(
                    path.to_path_buf(),
                    entry.as_ref().unwrap().oid.clone(),
                    stat,
                );
            }
        }

        Ok(())
    }

    fn insert_conflict(&mut self, conflict_type: ConflictType, path: &Path) {
        if let Some(conflicts) = self.conflicts.get_mut(&conflict_type) {
            conflicts.insert(path.to_path_buf());
        }
    }

    fn check_for_conflict(
        &mut self,
        path: &Path,
        old_item: &Option<Entry>,
        new_item: &Option<Entry>,
    ) -> Result<()> {
        let entry = self.repo.index.entry_for_path(&path_to_string(path), 0);

        if self.index_differs_from_trees(entry, old_item.as_ref(), new_item.as_ref()) {
            self.insert_conflict(ConflictType::StaleFile, path);
            return Ok(());
        }

        let stat = self.repo.workspace.stat_file(path)?;
        let error_type = self.get_error_type(stat.as_ref(), entry, new_item);

        if stat.is_none() {
            let parent = self.untracked_parent(path)?;
            if let Some(parent) = parent {
                let conflict_path = if entry.is_some() {
                    path.to_path_buf()
                } else {
                    parent
                };
                self.insert_conflict(error_type, &conflict_path);
            }
        } else if stat.as_ref().unwrap().is_file() {
            let changed = self.repo.compare_index_to_workspace(entry, stat.as_ref())?;
            if changed.is_some() {
                self.insert_conflict(error_type, path);
            }
        } else if stat.as_ref().unwrap().is_dir() {
            let trackable = self.repo.trackable_file(path, &stat.unwrap())?;
            if trackable {
                self.insert_conflict(error_type, path);
            }
        }

        Ok(())
    }

    fn index_differs_from_trees(
        &self,
        entry: Option<&IndexEntry>,
        old_item: Option<&Entry>,
        new_item: Option<&Entry>,
    ) -> bool {
        let old_item = old_item.map(|old_item| TreeEntry::Entry(old_item.clone()));
        let new_item = new_item.map(|new_item| TreeEntry::Entry(new_item.clone()));

        self.repo
            .compare_tree_to_index(old_item.as_ref(), entry)
            .is_some()
            && self
                .repo
                .compare_tree_to_index(new_item.as_ref(), entry)
                .is_some()
    }

    fn untracked_parent(&self, path: &Path) -> Result<Option<PathBuf>> {
        for parent in parent_directories(path) {
            if let Ok(Some(parent_stat)) = self.repo.workspace.stat_file(&parent) {
                if !parent_stat.is_file() {
                    continue;
                }

                if self.repo.trackable_file(&parent, &parent_stat)? {
                    return Ok(Some(parent.to_path_buf()));
                }
            }
        }
        Ok(None)
    }

    fn get_error_type(
        &self,
        stat: Option<&fs::Metadata>,
        entry: Option<&IndexEntry>,
        item: &Option<Entry>,
    ) -> ConflictType {
        if entry.is_some() {
            ConflictType::StaleFile
        } else if stat.is_some() && stat.unwrap().is_dir() {
            ConflictType::StaleDirectory
        } else if item.is_some() {
            ConflictType::UntrackedOverwritten
        } else {
            ConflictType::UntrackedRemoved
        }
    }

    fn collect_errors(&mut self) -> Result<()> {
        for (conflict_type, paths) in &self.conflicts {
            if paths.is_empty() {
                continue;
            }

            let (header, footer) = MESSAGES[conflict_type];

            let mut error = vec![header.to_string()];
            for name in paths {
                error.push(format!("\t{}", path_to_string(name)));
            }
            error.push(footer.to_string());

            self.errors.push(error.join("\n"));
        }

        if !self.errors.is_empty() {
            Err(Error::MigrationConflict)
        } else {
            Ok(())
        }
    }
}
