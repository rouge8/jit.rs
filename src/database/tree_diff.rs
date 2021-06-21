use crate::database::entry::Entry;
use crate::database::tree::{Tree, TreeEntry};
use crate::database::{Database, ParsedObject};
use crate::errors::Result;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

pub struct TreeDiff<'a> {
    database: &'a mut Database,
    pub changes: HashMap<PathBuf, (Option<Entry>, Option<Entry>)>,
}

impl<'a> TreeDiff<'a> {
    pub fn new(database: &'a mut Database) -> Self {
        Self {
            database,
            changes: HashMap::new(),
        }
    }

    pub fn compare_oids(
        &mut self,
        a: Option<String>,
        b: Option<String>,
        prefix: &Path,
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

        self.detect_deletions(&a_entries, &b_entries, &prefix)?;
        self.detect_additions(&a_entries, &b_entries, &prefix)?;

        Ok(())
    }

    fn oid_to_tree(&mut self, oid: &str) -> Result<Tree> {
        let tree_oid = match self.database.load(oid)? {
            ParsedObject::Commit(commit) => commit.tree.clone(),
            ParsedObject::Tree(tree) => return Ok(tree.to_owned()),
            ParsedObject::Blob(_) => unreachable!(),
        };

        match self.database.load(&tree_oid)? {
            ParsedObject::Tree(tree) => Ok(tree.to_owned()),
            _ => unreachable!(),
        }
    }

    fn detect_deletions(
        &mut self,
        a: &BTreeMap<PathBuf, TreeEntry>,
        b: &BTreeMap<PathBuf, TreeEntry>,
        prefix: &Path,
    ) -> Result<()> {
        for (name, entry) in a {
            let path = prefix.join(name);
            let other = b.get(name);

            if Some(entry) == other {
                continue;
            }

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
            self.compare_oids(tree_a, tree_b, &path)?;

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
                self.changes.insert(path, (blob_a, blob_b));
            }
        }

        Ok(())
    }

    fn detect_additions(
        &mut self,
        a: &BTreeMap<PathBuf, TreeEntry>,
        b: &BTreeMap<PathBuf, TreeEntry>,
        prefix: &Path,
    ) -> Result<()> {
        for (name, entry) in b {
            let path = prefix.join(name);
            let other = a.get(name);

            if other.is_some() {
                continue;
            }

            if !entry.is_tree() {
                match entry {
                    TreeEntry::Entry(entry) => {
                        self.changes.insert(path, (None, Some(entry.to_owned())));
                    }
                    TreeEntry::Tree(_) => unreachable!(),
                }
            } else {
                self.compare_oids(None, Some(entry.oid()), &path)?;
            }
        }

        Ok(())
    }
}
