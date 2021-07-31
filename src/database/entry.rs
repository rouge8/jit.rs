use crate::index;
use crate::util::is_executable;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Entry {
    pub oid: String,
    pub mode: u32,
}

impl Entry {
    pub fn new(oid: String, mode: u32) -> Self {
        Entry { oid, mode }
    }

    pub fn mode(&self) -> u32 {
        if is_executable(self.mode) {
            0o100755
        } else {
            self.mode
        }
    }
}

impl From<&index::Entry> for Entry {
    fn from(entry: &index::Entry) -> Self {
        Entry {
            oid: entry.oid.clone(),
            mode: entry.mode,
        }
    }
}
