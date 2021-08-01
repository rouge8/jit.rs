use crate::database::blob::Blob;
use crate::database::entry::Entry;
use crate::database::object::Object;
use crate::database::tree_diff::{Differ, TreeDiffChanges};
use crate::errors::Result;
use crate::merge::diff3;
use crate::merge::inputs::Inputs;
use crate::repository::Repository;
use crate::util::{parent_directories, path_to_string};
use std::collections::HashMap;
use std::path::Path;

pub struct Resolve<'a> {
    repo: &'a mut Repository,
    inputs: &'a Inputs,
    left_diff: TreeDiffChanges,
    right_diff: TreeDiffChanges,
    clean_diff: TreeDiffChanges,
    conflicts: HashMap<String, Vec<Option<Entry>>>,
    untracked: HashMap<String, Entry>,
    pub on_progress: fn(String),
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
            untracked: HashMap::new(),
            on_progress: |_info| (),
        }
    }

    pub fn execute(&mut self) -> Result<()> {
        self.prepare_tree_diffs()?;

        let mut migration = self.repo.migration(self.clean_diff.clone());
        migration.apply_changes()?;

        self.add_conflicts_to_index();
        self.write_untracked_files()?;

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
        self.untracked = HashMap::new();

        let right_diff = self.right_diff.clone();
        let left_diff = self.left_diff.clone();
        for (path, (old_item, new_item)) in right_diff {
            if new_item.is_some() {
                self.file_dir_conflict(&path, &left_diff, &self.inputs.left_name);
            }
            self.same_path_conflict(&path, old_item, new_item)?;
        }

        let right_diff = self.right_diff.clone();
        for (path, (_, new_item)) in left_diff {
            if new_item.is_some() {
                self.file_dir_conflict(&path, &right_diff, &self.inputs.right_name);
            }
        }

        Ok(())
    }

    fn same_path_conflict(
        &mut self,
        path: &Path,
        base: Option<Entry>,
        right: Option<Entry>,
    ) -> Result<()> {
        if self.conflicts.get(&path_to_string(path)).is_some() {
            return Ok(());
        }

        if !self.left_diff.contains_key(path) {
            self.clean_diff.insert(path.to_path_buf(), (base, right));
            return Ok(());
        }

        let left = self.left_diff[path].1.as_ref();
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

        if left.is_some() && right.is_some() {
            self.log(format!("Auto-merging {}", path_to_string(path)));
        }

        let (oid_ok, oid) = self.merge_blobs(
            base_oid.as_deref(),
            left_oid.as_deref(),
            right_oid.as_deref(),
        )?;
        let (mode_ok, mode) = self.merge_modes(base_mode, left_mode, right_mode);

        self.clean_diff.insert(
            path.to_path_buf(),
            (left.clone(), Some(Entry::new(oid, mode))),
        );

        if oid_ok && mode_ok {
            return Ok(());
        }

        self.conflicts
            .insert(path_to_string(path), vec![base, left, right]);
        self.log_conflict(path, None);

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

        let oids = vec![base_oid, left_oid, right_oid];
        let mut blobs = Vec::new();
        for oid in oids {
            if let Some(oid) = oid {
                let blob = self.repo.database.load_blob(oid)?;
                blobs.push(
                    std::str::from_utf8(&blob.data)
                        .expect("Invalid UTF-8")
                        .to_string(),
                );
            } else {
                blobs.push("".to_string());
            }
        }
        let blob_base = &blobs[0];
        let blob_left = &blobs[1];
        let blob_right = &blobs[2];
        let merge = diff3::merge(blob_base, blob_left, blob_right);

        let data = merge.to_string(Some(&self.inputs.left_name), Some(&self.inputs.right_name));
        let blob = Blob::new(data.as_bytes().to_vec());
        self.repo.database.store(&blob)?;

        Ok((merge.is_clean(), blob.oid()))
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

    fn file_dir_conflict(&mut self, path: &Path, diff: &TreeDiffChanges, name: &str) {
        for parent in parent_directories(path) {
            if !diff.contains_key(&parent) {
                continue;
            }

            let (old_item, new_item) = &diff[&parent];
            if new_item.is_none() {
                continue;
            }

            if name == self.inputs.left_name {
                self.conflicts.insert(
                    path_to_string(&parent),
                    vec![old_item.to_owned(), new_item.to_owned(), None],
                );
            } else if name == self.inputs.right_name {
                self.conflicts.insert(
                    path_to_string(&parent),
                    vec![old_item.to_owned(), None, new_item.to_owned()],
                );
            }

            self.clean_diff.remove(&parent);
            let rename = format!("{}~{}", path_to_string(&parent), name);
            self.untracked
                .insert(rename.clone(), new_item.to_owned().unwrap());

            if diff.get(path).is_none() {
                self.log(format!("Adding {}", path_to_string(path)));
            }
            self.log_conflict(&parent, Some(rename));
        }
    }

    fn add_conflicts_to_index(&mut self) {
        for (path, items) in &self.conflicts {
            self.repo.index.add_conflict_set(path, items.to_owned());
        }
    }

    fn write_untracked_files(&self) -> Result<()> {
        for (path, item) in &self.untracked {
            let blob = self.repo.database.load_blob(&item.oid)?;
            self.repo
                .workspace
                .write_file(Path::new(&path), blob.data)?;
        }

        Ok(())
    }

    fn log(&self, message: String) {
        (self.on_progress)(message);
    }

    fn log_conflict(&self, path: &Path, rename: Option<String>) {
        let path = path_to_string(path);
        let conflict = &self.conflicts[&path];
        let (base, left, right) = (&conflict[0], &conflict[1], &conflict[2]);

        if left.is_some() && right.is_some() {
            self.log_left_right_conflict(path);
        } else if base.is_some() && (left.is_some() || right.is_some()) {
            self.log_modify_delete_conflict(path, rename);
        } else {
            self.log_file_directory_conflict(path, rename.unwrap());
        }
    }

    fn log_left_right_conflict(&self, path: String) {
        let r#type = if self.conflicts[&path][0].is_some() {
            "content"
        } else {
            "add/add"
        };
        self.log(format!("CONFLICT ({}): Merge conflict in {}", r#type, path));
    }

    fn log_modify_delete_conflict(&self, path: String, rename: Option<String>) {
        let (deleted, modified) = self.log_branch_names(&path);

        let rename = if let Some(rename) = rename {
            format!(" at {}", rename)
        } else {
            String::new()
        };

        self.log(format!(
            "CONFLICT (modify/delete): {} deleted in {} and modified in {}. Version {} of {} left in tree{}.",
            path, deleted, modified, modified, path, rename,
        ));
    }

    fn log_file_directory_conflict(&self, path: String, rename: String) {
        let r#type = if self.conflicts[&path][1].is_some() {
            "file/directory"
        } else {
            "directory/file"
        };
        let (branch, _) = self.log_branch_names(&path);

        self.log(format!(
            "CONFLICT ({}): There is a directory with name {} in {}. Adding {} as {}",
            r#type, path, branch, path, rename,
        ));
    }

    fn log_branch_names(&self, path: &str) -> (String, String) {
        let (a, b) = (
            self.inputs.left_name.clone(),
            self.inputs.right_name.clone(),
        );

        if self.conflicts[path][1].is_some() {
            (b, a)
        } else {
            (a, b)
        }
    }
}
