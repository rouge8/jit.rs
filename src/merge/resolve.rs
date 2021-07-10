use crate::database::tree_diff::Differ;
use crate::errors::Result;
use crate::merge::inputs::Inputs;
use crate::repository::Repository;

pub struct Resolve<'a> {
    repo: &'a mut Repository,
    inputs: &'a Inputs,
}

impl<'a> Resolve<'a> {
    pub fn new(repo: &'a mut Repository, inputs: &'a Inputs) -> Self {
        Self { repo, inputs }
    }

    pub fn execute(&mut self) -> Result<()> {
        let base_oid = self.inputs.base_oids.first().map(String::as_str);
        let tree_diff =
            self.repo
                .database
                .tree_diff(base_oid, Some(&self.inputs.right_oid), None)?;
        let mut migration = self.repo.migration(tree_diff);

        migration.apply_changes()?;

        Ok(())
    }
}
