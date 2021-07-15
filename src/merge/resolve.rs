use crate::database::blob::Blob;
use crate::database::entry::Entry;
use crate::database::object::Object;
use crate::database::tree_diff::{Differ, TreeDiffChanges};
use crate::errors::Result;
use crate::merge::inputs::Inputs;
use crate::repository::Repository;
use crate::util::path_to_string;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct Resolve<'a> {
    repo: &'a mut Repository,
    inputs: &'a Inputs,
    left_diff: TreeDiffChanges,
    right_diff: TreeDiffChanges,
    clean_diff: TreeDiffChanges,
    conflicts: HashMap<String, Vec<Option<Entry>>>,
}

impl<'a> Resolve<'a> {
    pub fn new(repo: &'a mut Repository, inputs: &'a Inputs) -> Self {
        Self {
            repo,
            inputs,
            left_diff: TreeDiffChanges::new(),
            right_diff: TreeDiffChanges::new(),
            clean_diff: TreeDiffChanges::new(),
            conflicts: HashMap::new(),
        }
    }

    pub fn execute(&mut self) -> Result<()> {
        self.prepare_tree_diffs()?;

        let mut migration = self.repo.migration(self.clean_diff.clone());
        migration.apply_changes()?;

        self.add_conflicts_to_index();

        Ok(())
    }

    fn prepare_tree_diffs(&mut self) -> Result<()> {
        let base_oid = self.inputs.base_oids.first().map(String::as_str);
        self.left_diff =
            self.repo
                .database
                .tree_diff(base_oid, Some(&self.inputs.left_oid), None)?;
        self.right_diff =
            self.repo
                .database
                .tree_diff(base_oid, Some(&self.inputs.right_oid), None)?;
        self.clean_diff = TreeDiffChanges::new();
        self.conflicts = HashMap::new();

        let right_diff = self.right_diff.clone();
        for (path, (old_item, new_item)) in right_diff {
            self.same_path_conflict(path, old_item, new_item)?;
        }

        Ok(())
    }

    fn same_path_conflict(
        &mut self,
        path: PathBuf,
        base: Option<Entry>,
        right: Option<Entry>,
    ) -> Result<()> {
        if !self.left_diff.contains_key(&path) {
            self.clean_diff.insert(path, (base, right));
            return Ok(());
        }

        let left = self.left_diff[&path].1.as_ref();
        if left == right.as_ref() {
            return Ok(());
        }
        let left = left.map(|left| left.to_owned());

        let base_oid = base.as_ref().map(|base| base.oid.clone());
        let left_oid = left.as_ref().map(|left| left.oid.clone());
        let right_oid = right.as_ref().map(|right| right.oid.clone());

        let base_mode = base.as_ref().map(|base| base.mode);
        let left_mode = left.as_ref().map(|left| left.mode);
        let right_mode = right.as_ref().map(|right| right.mode);

        let (oid_ok, oid) = self.merge_blobs(
            base_oid.as_deref(),
            left_oid.as_deref(),
            right_oid.as_deref(),
        )?;
        let (mode_ok, mode) = self.merge_modes(base_mode, left_mode, right_mode);

        self.clean_diff.insert(
            path.clone(),
            (left.clone(), Some(Entry::new(&path, oid, mode))),
        );

        if !(oid_ok && mode_ok) {
            self.conflicts
                .insert(path_to_string(&path), vec![base, left, right]);
        }

        Ok(())
    }

    fn merge_blobs(
        &self,
        base_oid: Option<&str>,
        left_oid: Option<&str>,
        right_oid: Option<&str>,
    ) -> Result<(bool, String)> {
        let result = self.merge3(base_oid.as_ref(), left_oid.as_ref(), right_oid.as_ref());
        if let Some(result) = result {
            return Ok((result.0, result.1.to_string()));
        }

        let blob = Blob::new(self.merged_data(&left_oid.unwrap(), &right_oid.unwrap())?);
        self.repo.database.store(&blob)?;

        Ok((false, blob.oid()))
    }

    fn merged_data(&self, left_oid: &str, right_oid: &str) -> Result<Vec<u8>> {
        let mut left_blob = self.repo.database.load_blob(left_oid)?;
        let mut right_blob = self.repo.database.load_blob(right_oid)?;

        let mut result = vec![];
        result.extend_from_slice(format!("<<<<<<< {}\n", self.inputs.left_name).as_bytes());
        result.append(&mut left_blob.data);
        result.extend_from_slice(b"=======\n");
        result.append(&mut right_blob.data);
        result.extend_from_slice(format!(">>>>>>> {}\n", self.inputs.right_name).as_bytes());

        Ok(result)
    }

    fn merge_modes(
        &self,
        base_mode: Option<u32>,
        left_mode: Option<u32>,
        right_mode: Option<u32>,
    ) -> (bool, u32) {
        if let Some(result) = self.merge3(base_mode, left_mode, right_mode) {
            result
        } else {
            (false, left_mode.unwrap())
        }
    }

    fn merge3<T: Eq>(
        &self,
        base: Option<T>,
        left: Option<T>,
        right: Option<T>,
    ) -> Option<(bool, T)> {
        if left.is_none() {
            return Some((false, right.unwrap()));
        }
        if right.is_none() {
            return Some((false, left.unwrap()));
        }

        if left == base || left == right {
            return Some((true, right.unwrap()));
        } else if right == base {
            return Some((true, left.unwrap()));
        }

        None
    }

    fn add_conflicts_to_index(&mut self) {
        for (path, items) in &self.conflicts {
            self.repo.index.add_conflict_set(path, items.to_owned());
        }
    }
}
