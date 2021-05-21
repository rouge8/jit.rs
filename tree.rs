use crate::entry::Entry;
use crate::object::Object;
use hex;

#[derive(Debug)]
pub struct Tree {
    entries: Vec<Entry>,
}

impl Tree {
    pub fn new(mut entries: Vec<Entry>) -> Self {
        // Sort `entries` for `fmt::Display`
        entries.sort_by(|a, b| a.name.cmp(&b.name));

        Tree { entries }
    }
}

impl Object for Tree {
    fn r#type(&self) -> &str {
        "tree"
    }

    fn bytes(&self) -> Vec<u8> {
        // Relies on `self.entries` already being sorted
        let mut content = Vec::new();

        for entry in &self.entries {
            content.append(&mut format!("100644 {}\0", entry.name).into_bytes());
            content.append(&mut hex::decode(&entry.oid).unwrap());
        }

        content
    }
}
