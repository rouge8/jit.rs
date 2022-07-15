use std::collections::HashSet;

use crate::database::Database;
use crate::errors::Result;
use crate::merge::common_ancestors::{CommonAncestors, Flag};

pub struct Bases<'a> {
    database: &'a Database,
    common: CommonAncestors<'a>,
    commits: Vec<String>,
    redundant: HashSet<String>,
}

impl<'a> Bases<'a> {
    pub fn new(database: &'a Database, one: &str, two: &str) -> Result<Self> {
        Ok(Self {
            database,
            common: CommonAncestors::new(database, one, &[two])?,
            commits: Vec::new(),
            redundant: HashSet::new(),
        })
    }

    pub fn find(&mut self) -> Result<Vec<String>> {
        self.commits = self.common.find()?;
        if self.commits.len() <= 1 {
            return Ok(self.commits.clone());
        }

        self.redundant = HashSet::new();

        // We have to iterate over `commits` rather than `self.commits` because Rust will not let
        // us borrow self as mutable in `self.filter_commit()` and borrow as immutable while
        // iterating over `self.commits`.
        let commits = self.commits.to_vec();
        for commit in commits {
            self.filter_commit(&commit)?;
        }

        Ok(self
            .commits
            .iter()
            .filter_map(|commit| {
                if !self.redundant.contains(commit) {
                    Some(commit.to_owned())
                } else {
                    None
                }
            })
            .collect())
    }

    fn filter_commit(&mut self, commit: &str) -> Result<()> {
        if self.redundant.contains(commit) {
            return Ok(());
        }

        let others: Vec<_> = self
            .commits
            .iter()
            .filter_map(|oid| {
                if oid == commit || self.redundant.contains(oid) {
                    None
                } else {
                    Some(oid.as_str())
                }
            })
            .collect();
        let mut common = CommonAncestors::new(self.database, commit, &others)?;

        common.find()?;

        if common.is_marked(commit.to_string(), Flag::Parent2) {
            self.redundant.insert(commit.to_string());
        }

        let others = others
            .iter()
            .filter(|oid| common.is_marked(oid.to_string(), Flag::Parent1));
        for oid in others {
            self.redundant.insert(oid.to_string());
        }

        Ok(())
    }
}
