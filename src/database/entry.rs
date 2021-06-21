use crate::index;
use crate::util::basename;
use crate::util::is_executable;
use crate::util::parent_directories;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq, Clone)]
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

    pub fn mode(&self) -> u32 {
        if is_executable(self.mode) {
            0o100755
        } else {
            self.mode
        }
    }

    pub fn basename(&self) -> PathBuf {
        basename(PathBuf::from(&self.name))
    }

    pub fn parent_directories(&self) -> Vec<PathBuf> {
        parent_directories(PathBuf::from(&self.name))
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
