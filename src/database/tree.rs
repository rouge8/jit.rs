use crate::database::entry::Entry as DatabaseEntry;
use crate::database::object::Object;
use crate::database::ParsedObject;
use crate::index::Entry as IndexEntry;
use crate::util::path_to_string;
use itertools::Itertools;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const TREE_MODE: u32 = 0o40000;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Tree {
    pub entries: BTreeMap<PathBuf, TreeEntry>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TreeEntry {
    Entry(DatabaseEntry),
    Tree(Tree),
}

impl TreeEntry {
    pub fn mode(&self) -> u32 {
        match self {
            TreeEntry::Entry(e) => e.mode(),
            TreeEntry::Tree(_) => TREE_MODE,
        }
    }

    pub fn oid(&self) -> String {
        match self {
            TreeEntry::Entry(e) => e.oid.clone(),
            TreeEntry::Tree(e) => e.oid(),
        }
    }

    pub fn is_tree(&self) -> bool {
        match self {
            TreeEntry::Entry(e) => e.mode() == TREE_MODE,
            TreeEntry::Tree(_) => true,
        }
    }
}

impl Tree {
    pub fn new(entries: Option<BTreeMap<PathBuf, TreeEntry>>) -> Self {
        let entries = if let Some(entries) = entries {
            entries
        } else {
            BTreeMap::new()
        };

        Tree { entries }
    }

    pub fn parse(data: &[u8]) -> ParsedObject {
        let mut entries = BTreeMap::new();

        let mut data = data;

        while !data.is_empty() {
            let (mode, rest) = data
                .splitn(2, |c| *c as char == ' ')
                .collect_tuple()
                .unwrap();
            // TODO: There has to be a better way to do this...
            let mode = u32::from_str_radix(std::str::from_utf8(mode).unwrap(), 8).unwrap();

            let (name, rest) = rest
                .splitn(2, |c| *c as char == '\0')
                .collect_tuple()
                .unwrap();
            let name = std::str::from_utf8(name).unwrap();

            let (oid, rest) = rest.split_at(20);
            let oid = hex::encode(oid);

            entries.insert(
                PathBuf::from(name),
                TreeEntry::Entry(DatabaseEntry::new(oid, mode)),
            );

            data = rest;
        }

        ParsedObject::Tree(Tree::new(Some(entries)))
    }

    pub fn build(entries: Vec<IndexEntry>) -> Self {
        let mut root = Tree::new(None);
        for entry in entries {
            root.add_entry(entry.parent_directories(), entry);
        }

        root
    }

    pub fn traverse<F>(&self, f: &F)
    where
        F: Fn(&Tree),
    {
        for entry in self.entries.values() {
            match entry {
                TreeEntry::Tree(e) => e.traverse(f),
                TreeEntry::Entry(_) => (),
            }
        }

        f(self);
    }

    fn add_entry(&mut self, parents: Vec<PathBuf>, entry: IndexEntry) {
        if parents.is_empty() {
            self.entries.insert(
                entry.basename(),
                TreeEntry::Entry(DatabaseEntry::from(&entry)),
            );
        } else {
            let key = PathBuf::from(parents[0].file_name().unwrap());
            let new_parents = parents[1..].to_vec();

            if let Some(TreeEntry::Tree(tree)) = self.entries.get_mut(&key) {
                tree.add_entry(new_parents, entry);
            } else {
                let mut tree = Tree::new(None);
                tree.add_entry(new_parents, entry);
                self.entries.insert(key, TreeEntry::Tree(tree));
            }
        }
    }
}

impl Object for Tree {
    fn r#type(&self) -> &str {
        "tree"
    }

    fn bytes(&self) -> Vec<u8> {
        let mut content = Vec::new();

        // Sort `self.entries` by name with tree names being treated like `$name/` (with a trailing
        // slash). This makes `foo.txt` sort before `foo/` before `foo:txt` which matches Git's
        // behavior.
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by_key(|(name, entry)| {
            let mut name = path_to_string(name);

            if entry.is_tree() {
                name.push('/');
            }

            name
        });

        for (name, entry) in entries {
            content
                .append(&mut format!("{:o} {}\0", entry.mode(), path_to_string(name)).into_bytes());
            content.append(&mut hex::decode(entry.oid()).unwrap());
        }
        content
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_bytes_sort_order() {
        let mut entries = BTreeMap::new();
        // Use "" for oids so they don't clutter the serialized tree
        entries.insert(
            PathBuf::from("test:txt"),
            TreeEntry::Entry(DatabaseEntry::new(String::from(""), 0o100644)),
        );
        entries.insert(
            PathBuf::from("test.txt"),
            TreeEntry::Entry(DatabaseEntry::new(String::from(""), 0o100644)),
        );
        entries.insert(
            PathBuf::from("test"),
            TreeEntry::Entry(DatabaseEntry::new(String::from(""), TREE_MODE)),
        );
        let tree = Tree::new(Some(entries));

        let bytes = tree.bytes();
        let serialized = std::str::from_utf8(&bytes).unwrap();

        assert_eq!(serialized, "100644 test.txt\040000 test\0100644 test:txt\0");
    }
}
