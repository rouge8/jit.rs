use crate::database::entry::Entry;
use crate::database::object::Object;
use crate::database::ParsedObject;
use itertools::Itertools;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const TREE_MODE: u32 = 0o40000;

#[derive(Debug, Clone)]
pub struct Tree {
    pub entries: BTreeMap<PathBuf, TreeEntry>,
}

#[derive(Debug, Clone)]
pub enum TreeEntry {
    Entry(Entry),
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
            _ => false,
        }
    }
}

impl Tree {
    pub fn new() -> Self {
        Tree {
            entries: BTreeMap::new(),
        }
    }

    pub fn parse(data: &[u8]) -> ParsedObject {
        let mut entries: Vec<Entry> = vec![];

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

            entries.push(Entry::new(Path::new(name), oid, mode));

            data = rest;
        }

        ParsedObject::Tree(Tree::build(entries))
    }

    pub fn build(entries: Vec<Entry>) -> Self {
        let mut root = Tree::new();
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

    fn add_entry(&mut self, parents: Vec<PathBuf>, entry: Entry) {
        if parents.is_empty() {
            self.entries
                .insert(entry.basename(), TreeEntry::Entry(entry));
        } else {
            let key = PathBuf::from(parents[0].file_name().unwrap());
            let new_parents = parents[1..].to_vec();

            if let Some(TreeEntry::Tree(tree)) = self.entries.get_mut(&key) {
                tree.add_entry(new_parents, entry);
            } else {
                let mut tree = Tree::new();
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
        // Relies on `self.entries` already being sorted
        let mut content = Vec::new();

        for (name, entry) in self.entries.iter() {
            // TODO: Figure out how to get bytes from `name` instead of calling
            // `name.display()` which is lossy.
            content.append(&mut format!("{:o} {}\0", entry.mode(), name.display()).into_bytes());
            content.append(&mut hex::decode(entry.oid()).unwrap());
        }
        content
    }
}
