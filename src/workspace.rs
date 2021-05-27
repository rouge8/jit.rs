use crate::errors::{Error, Result};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

// TODO: Remove `target` once we have .gitignore support
const IGNORE: &[&str] = &[".", "..", ".git", "target"];

#[derive(Debug)]
pub struct Workspace {
    pathname: PathBuf,
}

impl Workspace {
    pub fn new(pathname: PathBuf) -> Self {
        Workspace { pathname }
    }

    pub fn list_files(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let relative_path = path.strip_prefix(&self.pathname).unwrap();

        if self.should_ignore(&relative_path) {
            Ok(vec![])
        } else if relative_path.is_file() {
            Ok(vec![relative_path.to_path_buf()])
        } else {
            let mut files: Vec<PathBuf> = Vec::new();

            for entry in fs::read_dir(&path)? {
                let path = entry?.path();
                let mut nested = self.list_files(&path)?;
                files.append(&mut nested);
            }
            Ok(files)
        }
    }

    pub fn read_file(&self, path: &Path) -> Result<Vec<u8>> {
        fs::read(&self.pathname.join(&path)).map_err(|err| {
            if err.kind() == io::ErrorKind::PermissionDenied {
                Error::NoPermission {
                    method: String::from("open"),
                    path: path.to_path_buf(),
                }
            } else {
                Error::Io(err)
            }
        })
    }

    pub fn stat_file(&self, path: &Path) -> Result<fs::Metadata> {
        fs::metadata(&self.pathname.join(&path)).map_err(|err| {
            if err.kind() == io::ErrorKind::PermissionDenied {
                Error::NoPermission {
                    method: String::from("stat"),
                    path: path.to_path_buf(),
                }
            } else {
                Error::Io(err)
            }
        })
    }

    fn should_ignore(&self, path: &Path) -> bool {
        IGNORE
            .iter()
            .any(|ignore_path| path == PathBuf::from(ignore_path))
    }
}
