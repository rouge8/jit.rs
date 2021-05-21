use crate::entry::Entry;
use crate::object::Object;
use hex;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Tree {
    entries: BTreeMap<PathBuf, TreeEntry>,
}

#[derive(Debug)]
pub enum TreeEntry {
    Entry(Entry),
    Tree(Tree),
}

impl TreeEntry {
    fn mode(&self) -> &str {
        match self {
            TreeEntry::Entry(e) => e.mode(),
            TreeEntry::Tree(_) => "40000",
        }
    }

    fn oid(&self) -> String {
        match self {
            TreeEntry::Entry(e) => e.oid.clone(),
            TreeEntry::Tree(e) => e.oid(),
        }
    }
}

impl Tree {
    pub fn new() -> Self {
        Tree {
            entries: BTreeMap::new(),
        }
    }

    pub fn build(mut entries: Vec<Entry>) -> Self {
        // Sort `entries` for `fmt::Display`
        entries.sort_by(|a, b| a.name.cmp(&b.name));

        let mut root = Tree::new();
        for entry in entries {
            root.add_entry(entry.parent_directories(), entry);
        }

        root
    }

    pub fn traverse<F>(&self, f: &F)
    where
        F: Fn(&Tree) -> (),
    {
        for entry in self.entries.values() {
            match entry {
                TreeEntry::Tree(e) => e.traverse(f),
                TreeEntry::Entry(_) => (),
            }
        }

        f(self);
    }

    fn add_entry(&mut self, parents: Vec<PathBuf>, entry: Entry) {
        if parents.is_empty() {
            &self
                .entries
                .insert(entry.basename(), TreeEntry::Entry(entry));
        } else {
            let key = PathBuf::from(parents[0].file_name().unwrap());
            let new_parents = parents[1..].to_vec();

            if let Some(TreeEntry::Tree(tree)) = self.entries.get_mut(&key) {
                tree.add_entry(new_parents, entry);
            } else {
                let mut tree = Tree::new();
                tree.add_entry(new_parents, entry);
                &self.entries.insert(key, TreeEntry::Tree(tree));
            }
        }
    }
}

impl Object for Tree {
    fn r#type(&self) -> &str {
        "tree"
    }

    fn bytes(&self) -> Vec<u8> {
        // Relies on `self.entries` already being sorted
        let mut content = Vec::new();

        for (name, entry) in self.entries.iter() {
            // TODO: Figure out how to get bytes from `name` instead of calling
            // `name.display()` which is lossy.
            content.append(&mut format!("{} {}\0", entry.mode(), name.display()).into_bytes());
            content.append(&mut hex::decode(entry.oid()).unwrap());
        }
        content
    }
}
