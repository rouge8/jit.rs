use std::path::{Path, PathBuf};

use crate::errors::Result;
use crate::repository::status::Status;
use crate::repository::Repository;
use crate::util::path_to_string;

pub struct HardReset<'a> {
    repo: &'a mut Repository,
    status: Status,
}

impl<'a> HardReset<'a> {
    pub fn new(repo: &'a mut Repository, oid: &str) -> Self {
        let status = repo.status(Some(oid));

        Self { repo, status }
    }

    pub fn execute(&mut self) -> Result<()> {
        self.status.initialize()?;
        let changed = self
            .status
            .changed
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();

        for path in &changed {
            self.reset_path(path)?;
        }

        Ok(())
    }

    fn reset_path(&mut self, path: &Path) -> Result<()> {
        self.repo.index.remove(path);
        self.repo.workspace.remove(path)?;

        let entry = self.status.head_tree.get(&path_to_string(path));
        if let Some(entry) = entry {
            let blob = self.repo.database.load_blob(&entry.oid())?;
            self.repo
                .workspace
                .write_file(path, blob.data, Some(entry.mode()), true)?;

            let stat = self.repo.workspace.stat_file(path)?.unwrap();
            self.repo.index.add(path.to_path_buf(), entry.oid(), stat);
        }

        Ok(())
    }
}
