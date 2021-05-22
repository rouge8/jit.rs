use crate::lockfile::Lockfile;
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Refs {
    pathname: PathBuf,
}

impl Refs {
    pub fn new(pathname: PathBuf) -> Self {
        Refs { pathname }
    }

    pub fn update_head(&self, oid: String) -> Result<()> {
        let mut lockfile = Lockfile::new(self.head_path());
        lockfile.hold_for_update()?;

        lockfile.write(&format!("{}\n", oid).into_bytes())?;
        lockfile.commit()
    }

    pub fn read_head(&self) -> Option<String> {
        let path = self.head_path();
        if path.exists() {
            Some(fs::read_to_string(path).unwrap().trim().to_string())
        } else {
            None
        }
    }

    fn head_path(&self) -> PathBuf {
        self.pathname.join("HEAD")
    }
}
