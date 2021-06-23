use crate::database::entry::Entry;
use crate::database::tree_diff::TreeDiffChanges;
use crate::database::ParsedObject;
use crate::errors::Result;
use crate::repository::Repository;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

pub struct Migration<'a> {
    repo: &'a mut Repository,
    diff: TreeDiffChanges,
    pub changes: HashMap<Action, Vec<(PathBuf, Option<Entry>)>>,
    pub mkdirs: BTreeSet<PathBuf>,
    pub rmdirs: BTreeSet<PathBuf>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Action {
    Create,
    Delete,
    Update,
}

impl<'a> Migration<'a> {
    pub fn new(repo: &'a mut Repository, diff: TreeDiffChanges) -> Self {
        let changes = {
            let mut changes = HashMap::new();
            changes.insert(Action::Create, vec![]);
            changes.insert(Action::Delete, vec![]);
            changes.insert(Action::Update, vec![]);

            changes
        };

        Self {
            repo,
            diff,
            changes,
            mkdirs: BTreeSet::new(),
            rmdirs: BTreeSet::new(),
        }
    }

    pub fn apply_changes(&mut self) -> Result<()> {
        self.plan_changes()?;
        self.update_workspace()?;
        self.update_index()?;

        Ok(())
    }

    pub fn blob_data(&self, oid: &str) -> Result<Vec<u8>> {
        // We use `read_object()` instead of `load()` here in order to avoid writing the object to
        // `self.repo.database`, which would make this method and all of its callers require
        // borrowing `&mut self`.
        match self.repo.database.read_object(oid)? {
            ParsedObject::Blob(blob) => Ok(blob.data),
            _ => unreachable!(),
        }
    }

    fn plan_changes(&mut self) -> Result<()> {
        // TODO: Pass `diff` as an argument to `apply_changes()` instead of cloning?
        for (path, (old_item, new_item)) in &self.diff.clone() {
            self.record_change(path, old_item, new_item);
        }

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
        self.repo.workspace.apply_migration(&self)?;
        Ok(())
    }

    fn update_index(&mut self) -> Result<()> {
        for (path, _) in &self.changes[&Action::Delete] {
            self.repo.index.remove(path);
        }

        for action in [Action::Create, Action::Update] {
            for (path, entry) in &self.changes[&action] {
                let stat = self.repo.workspace.stat_file(path)?;
                self.repo.index.add(
                    path.to_path_buf(),
                    entry.as_ref().unwrap().oid.clone(),
                    stat,
                );
            }
        }

        Ok(())
    }
}
