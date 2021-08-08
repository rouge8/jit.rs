use crate::errors::{Error, Result};
use std::fs;
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
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

    pub fn in_progress(&self) -> bool {
        self.message_path.exists()
    }

    pub fn merge_oid(&self) -> Result<String> {
        match fs::read_to_string(&self.head_path) {
            Ok(oid) => Ok(oid.trim().to_string()),
            Err(err) => {
                if err.kind() == io::ErrorKind::NotFound {
                    let name = self
                        .head_path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .to_string();

                    Err(Error::NoMergeInProgress(name))
                } else {
                    Err(Error::Io(err))
                }
            }
        }
    }

    pub fn merge_message(&self) -> Result<String> {
        let message = fs::read_to_string(&self.message_path)?;

        Ok(message)
    }

    pub fn clear(&self) -> Result<()> {
        match fs::remove_file(&self.head_path) {
            Ok(()) => (),
            Err(err) => return self.handle_no_merge_to_abort(err),
        }
        match fs::remove_file(&self.message_path) {
            Ok(()) => (),
            Err(err) => return self.handle_no_merge_to_abort(err),
        }

        Ok(())
    }

    fn handle_no_merge_to_abort(&self, err: io::Error) -> Result<()> {
        if err.kind() == io::ErrorKind::NotFound {
            let name = self
                .head_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            Err(Error::NoMergeToAbort(name))
        } else {
            Err(Error::Io(err))
        }
    }
}
