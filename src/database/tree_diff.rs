use crate::database::entry::Entry;
use crate::database::tree::{Tree, TreeEntry};
use crate::database::{Database, ParsedObject};
use crate::errors::Result;
use crate::path_filter::PathFilter;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

pub type TreeDiffChanges = HashMap<PathBuf, (Option<Entry>, Option<Entry>)>;

pub trait Differ {
    fn tree_diff(
        &self,
        a: Option<&str>,
        b: Option<&str>,
        filter: Option<&PathFilter>,
    ) -> Result<TreeDiffChanges>;
}

pub struct TreeDiff<'a> {
    database: &'a Database,
    pub changes: TreeDiffChanges,
}

impl<'a> TreeDiff<'a> {
    pub fn new(database: &'a Database) -> Self {
        Self {
            database,
            changes: HashMap::new(),
        }
    }

    pub fn compare_oids(
        &mut self,
        a: Option<&str>,
        b: Option<&str>,
        filter: &PathFilter,
    ) -> Result<()> {
        if a == b {
            return Ok(());
        }

        let a_entries = if let Some(a_oid) = a {
            self.oid_to_tree(&a_oid)?.entries
        } else {
            BTreeMap::new()
        };
        let b_entries = if let Some(b_oid) = b {
            self.oid_to_tree(&b_oid)?.entries
        } else {
            BTreeMap::new()
        };

        self.detect_deletions(&a_entries, &b_entries, &filter)?;
        self.detect_additions(&a_entries, &b_entries, &filter)?;

        Ok(())
    }

    fn oid_to_tree(&self, oid: &str) -> Result<Tree> {
        let tree_oid = match self.database.load(oid)? {
            ParsedObject::Commit(commit) => commit.tree,
            ParsedObject::Tree(tree) => return Ok(tree),
            ParsedObject::Blob(_) => unreachable!(),
        };

        match self.database.load(&tree_oid)? {
            ParsedObject::Tree(tree) => Ok(tree),
            _ => unreachable!(),
        }
    }

    fn detect_deletions(
        &mut self,
        a: &BTreeMap<PathBuf, TreeEntry>,
        b: &BTreeMap<PathBuf, TreeEntry>,
        filter: &PathFilter,
    ) -> Result<()> {
        for (name, entry) in filter.each_entry(a) {
            let other = b.get(&name);

            if Some(&entry) == other {
                continue;
            }

            let sub_filter = filter.join(name);

            let tree_a = if entry.is_tree() {
                Some(entry.oid())
            } else {
                None
            };
            let tree_b = if let Some(other) = other {
                if other.is_tree() {
                    Some(other.oid())
                } else {
                    None
                }
            } else {
                None
            };
            self.compare_oids(tree_a.as_deref(), tree_b.as_deref(), &sub_filter)?;

            let blob_a = if entry.is_tree() {
                None
            } else {
                match entry {
                    TreeEntry::Entry(entry) => Some(entry.to_owned()),
                    TreeEntry::Tree(_) => unreachable!(),
                }
            };
            let blob_b = if let Some(other) = other {
                if other.is_tree() {
                    None
                } else {
                    match other {
                        TreeEntry::Entry(other) => Some(other.to_owned()),
                        TreeEntry::Tree(_) => unreachable!(),
                    }
                }
            } else {
                None
            };

            if blob_a.is_some() || blob_b.is_some() {
                self.changes
                    .insert(sub_filter.path.clone(), (blob_a, blob_b));
            }
        }

        Ok(())
    }

    fn detect_additions(
        &mut self,
        a: &BTreeMap<PathBuf, TreeEntry>,
        b: &BTreeMap<PathBuf, TreeEntry>,
        filter: &PathFilter,
    ) -> Result<()> {
        for (name, entry) in filter.each_entry(b) {
            let other = a.get(&name);

            if other.is_some() {
                continue;
            }

            let sub_filter = filter.join(name);

            if !entry.is_tree() {
                match entry {
                    TreeEntry::Entry(entry) => {
                        self.changes
                            .insert(sub_filter.path.clone(), (None, Some(entry.to_owned())));
                    }
                    TreeEntry::Tree(_) => unreachable!(),
                }
            } else {
                self.compare_oids(None, Some(&entry.oid()), &sub_filter)?;
            }
        }

        Ok(())
    }
}
