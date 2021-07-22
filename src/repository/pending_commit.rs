use crate::errors::Result;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub struct PendingCommit {
    head_path: PathBuf,
    message_path: PathBuf,
}

impl PendingCommit {
    pub fn new(pathname: &Path) -> Self {
        Self {
            head_path: pathname.join("MERGE_HEAD"),
            message_path: pathname.join("MERGE_MSG"),
        }
    }

    pub fn start(&self, oid: &str, message: &str) -> Result<()> {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.head_path)?
            .write_all(oid.as_bytes())?;
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.message_path)?
            .write_all(message.as_bytes())?;

        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        fs::remove_file(&self.head_path)?;
        fs::remove_file(&self.message_path)?;

        Ok(())
    }

    pub fn merge_message(&self) -> Result<String> {
        let mut message = String::new();
        File::open(&self.message_path)?.read_to_string(&mut message)?;

        Ok(message)
    }
}
