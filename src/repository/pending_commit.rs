use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{fs, io};

use once_cell::sync::Lazy;

use crate::errors::{Error, Result};

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum PendingCommitType {
    Merge,
    CherryPick,
    Revert,
}

static HEAD_FILES: Lazy<HashMap<PendingCommitType, &'static str>> = Lazy::new(|| {
    HashMap::from([
        (PendingCommitType::Merge, "MERGE_HEAD"),
        (PendingCommitType::CherryPick, "CHERRY_PICK_HEAD"),
        (PendingCommitType::Revert, "REVERT_HEAD"),
    ])
});

#[derive(Debug)]
pub struct PendingCommit {
    pathname: PathBuf,
    pub message_path: PathBuf,
}

impl PendingCommit {
    pub fn new(pathname: &Path) -> Self {
        Self {
            pathname: pathname.to_owned(),
            message_path: pathname.join("MERGE_MSG"),
        }
    }

    pub fn start(&self, oid: &str, r#type: PendingCommitType) -> Result<()> {
        let path = self.pathname.join(HEAD_FILES[&r#type]);
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?
            .write_all(oid.as_bytes())?;

        Ok(())
    }

    pub fn in_progress(&self) -> bool {
        self.merge_type() != None
    }

    pub fn merge_type(&self) -> Option<PendingCommitType> {
        for (r#type, name) in &*HEAD_FILES {
            let path = self.pathname.join(name);

            if path.exists() {
                return Some(*r#type);
            }
        }

        None
    }

    pub fn merge_oid(&self, r#type: PendingCommitType) -> Result<String> {
        let head_path = self.pathname.join(HEAD_FILES[&r#type]);

        match fs::read_to_string(&head_path) {
            Ok(oid) => Ok(oid.trim().to_string()),
            Err(err) => {
                if err.kind() == io::ErrorKind::NotFound {
                    let name = head_path.file_name().unwrap().to_string_lossy().to_string();

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

    pub fn clear(&self, r#type: PendingCommitType) -> Result<()> {
        let head_path = self.pathname.join(HEAD_FILES[&r#type]);

        match fs::remove_file(&head_path) {
            Ok(()) => (),
            Err(err) => return self.handle_no_merge_to_abort(&head_path, err),
        }
        fs::remove_file(&self.message_path)?;

        Ok(())
    }

    fn handle_no_merge_to_abort(&self, head_path: &Path, err: io::Error) -> Result<()> {
        if err.kind() == io::ErrorKind::NotFound {
            let name = head_path.file_name().unwrap().to_string_lossy().to_string();
            Err(Error::NoMergeToAbort(name))
        } else {
            Err(Error::Io(err))
        }
    }
}
