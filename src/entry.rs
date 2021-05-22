use std::path::PathBuf;

#[derive(Debug)]
pub struct Entry {
    pub name: String,
    pub oid: String,
    mode: u32,
}

impl Entry {
    pub fn new(name: &PathBuf, oid: String, mode: u32) -> Self {
        let name = name.to_str().unwrap().to_string();
        Entry { name, oid, mode }
    }

    pub fn mode(&self) -> &str {
        // Check if the mode is executable
        if (self.mode & 0o111) != 0 {
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