use crate::database::entry::Entry;
use crate::database::tree_diff::TreeDiffChanges;
use crate::errors::Result;
use crate::repository::Repository;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub struct Migration<'a> {
    repo: &'a Repository,
    diff: TreeDiffChanges,
    changes: HashMap<Action, Vec<(PathBuf, Option<Entry>)>>,
    mkdirs: HashSet<PathBuf>,
    rmdirs: HashSet<PathBuf>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum Action {
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
            mkdirs: HashSet::new(),
            rmdirs: HashSet::new(),
        }
    }

    pub fn apply_changes(&mut self) -> Result<()> {
        self.plan_changes()?;
        self.update_workspace()?;

        Ok(())
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
        Ok(())
    }
}
