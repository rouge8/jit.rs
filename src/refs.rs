use crate::errors::{Error, Result};
use crate::lockfile::Lockfile;
use crate::revision::Revision;
use std::fs;
use std::io;
use std::path::PathBuf;

const HEAD: &str = "HEAD";

#[derive(Debug)]
pub struct Refs {
    pathname: PathBuf,
    refs_path: PathBuf,
    heads_path: PathBuf,
}

impl Refs {
    pub fn new(pathname: PathBuf) -> Self {
        let refs_path = pathname.join("refs");
        let heads_path = pathname.join("heads");

        Refs {
            pathname,
            refs_path,
            heads_path,
        }
    }

    pub fn update_head(&self, oid: String) -> Result<()> {
        self.update_ref_file(self.pathname.join(HEAD), oid)
    }

    pub fn read_head(&self) -> Result<Option<String>> {
        self.read_ref_file(self.pathname.join(HEAD))
    }

    pub fn read_ref(&self, name: &str) -> Result<Option<String>> {
        if let Some(path) = self.path_for_name(name) {
            self.read_ref_file(path)
        } else {
            Ok(None)
        }
    }

    pub fn create_branch(&self, branch_name: &str) -> Result<()> {
        let path = self.heads_path.join(branch_name);

        if !Revision::valid_ref(branch_name) {
            return Err(Error::InvalidBranch(format!(
                "'{}' is not a valid branch name.",
                branch_name
            )));
        }

        if path.as_path().exists() {
            return Err(Error::InvalidBranch(format!(
                "A branch named '{}' already exists.",
                branch_name
            )));
        }

        self.update_ref_file(path, self.read_head()?.unwrap())?;

        Ok(())
    }

    fn path_for_name(&self, name: &str) -> Option<PathBuf> {
        let prefixes = [
            self.pathname.clone(),
            self.refs_path.clone(),
            self.heads_path.clone(),
        ];

        for prefix in &prefixes {
            if prefix.join(name).exists() {
                return Some(prefix.join(name));
            }
        }
        None
    }

    fn read_ref_file(&self, path: PathBuf) -> Result<Option<String>> {
        if path.exists() {
            Ok(Some(fs::read_to_string(path)?.trim().to_string()))
        } else {
            Ok(None)
        }
    }

    fn update_ref_file(&self, path: PathBuf, oid: String) -> Result<()> {
        let mut lockfile = Lockfile::new(path.clone());

        match lockfile.hold_for_update() {
            Ok(()) => (),
            Err(err) => match err {
                Error::Io(err) => {
                    if err.kind() == io::ErrorKind::NotFound {
                        // Create the parent directories and retry
                        fs::create_dir_all(path.parent().unwrap())?;
                        lockfile.hold_for_update()?;
                    } else {
                        return Err(Error::Io(err));
                    }
                }
                _ => return Err(err),
            },
        }
        lockfile.write(&format!("{}\n", oid).into_bytes())?;
        lockfile.commit()?;

        Ok(())
    }
}
