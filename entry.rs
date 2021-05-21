use std::path::PathBuf;

#[derive(Debug)]
pub struct Entry {
    pub name: String,
    pub oid: String,
}

impl Entry {
    pub fn new(name: &PathBuf, oid: String) -> Self {
        let name = name.to_str().unwrap().to_string();
        Entry { name, oid }
    }
}
