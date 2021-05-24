use crate::index;
use crate::util::is_executable;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Entry {
    pub name: String,
    pub oid: String,
    mode: u32,
}

impl Entry {
    pub fn new(name: &Path, oid: String, mode: u32) -> Self {
        let name = name.to_str().unwrap().to_string();
        Entry { name, oid, mode }
    }

    pub fn mode(&self) -> &str {
        if is_executable(self.mode) {
            "100755"
        } else {
            "100644"
        }
    }

    pub fn basename(&self) -> PathBuf {
        PathBuf::from(PathBuf::from(&self.name).file_name().unwrap())
    }

    pub fn parent_directories(&self) -> Vec<PathBuf> {
        let mut parents = Vec::new();
        let mut path = PathBuf::from(&self.name);

        // TODO: path.ancestors()
        while let Some(parent) = path.parent() {
            let parent = parent.to_path_buf();
            path = parent.clone();

            if parent != PathBuf::from("") {
                parents.insert(0, parent);
            }
        }

        parents
    }
}

impl From<&index::Entry> for Entry {
    fn from(entry: &index::Entry) -> Self {
        Entry {
            name: entry.path.clone(),
            oid: entry.oid.clone(),
            mode: entry.mode,
        }
    }
}
