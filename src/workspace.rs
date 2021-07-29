use crate::errors::{Error, Result};
use crate::repository::migration::{Action, Migration};
use nix::errno::Errno;
use std::collections::HashMap;
use std::fs;
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
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
        } else if path.is_file() {
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

    pub fn list_dir(&self, dirname: &Path) -> Result<HashMap<PathBuf, fs::Metadata>> {
        let path = self.pathname.join(dirname);
        let mut stats = HashMap::new();

        for entry in fs::read_dir(&path)? {
            let path = entry?.path();
            let relative_path = path.strip_prefix(&self.pathname).unwrap();

            if !self.should_ignore(&relative_path) {
                stats.insert(relative_path.to_path_buf(), self.stat_file(&relative_path)?);
            }
        }

        Ok(stats)
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

    pub fn write_file(&self, path: &Path, data: Vec<u8>) -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.pathname.join(path))?;
        file.write_all(&data)?;

        Ok(())
    }

    pub fn remove(&self, path: &Path) -> Result<()> {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) => {
                if err.kind() == io::ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(Error::Io(err))
                }
            }
        }
    }

    pub fn apply_migration(&self, migration: &Migration) -> Result<()> {
        self.apply_change_list(migration, Action::Delete)?;
        for dir in migration.rmdirs.iter().rev() {
            self.remove_directory(dir)?;
        }

        for dir in &migration.mkdirs {
            self.make_directory(dir)?;
        }
        self.apply_change_list(migration, Action::Update)?;
        self.apply_change_list(migration, Action::Create)?;

        Ok(())
    }

    fn should_ignore(&self, path: &Path) -> bool {
        IGNORE
            .iter()
            .any(|ignore_path| path == PathBuf::from(ignore_path))
    }

    fn apply_change_list(&self, migration: &Migration, action: Action) -> Result<()> {
        for (filename, entry) in &migration.changes[&action] {
            let path = self.pathname.join(filename);

            if path.is_dir() {
                fs::remove_dir_all(&path)?;
            } else if path.is_file() {
                fs::remove_file(&path)?;
            }
            if action == Action::Delete {
                continue;
            }

            let entry = entry.as_ref().unwrap();
            let data = migration.blob_data(&entry.oid)?;

            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            file.write_all(&data)?;

            let mut perms = fs::metadata(&path)?.permissions();
            perms.set_mode(entry.mode());
            fs::set_permissions(&path, perms)?;
        }

        Ok(())
    }

    fn remove_directory(&self, dirname: &Path) -> Result<()> {
        match fs::remove_dir(self.pathname.join(dirname)) {
            Ok(()) => Ok(()),
            Err(err) => {
                if err.kind() == io::ErrorKind::NotFound
                    || err.raw_os_error() == Some(Errno::ENOTDIR as i32)
                    || err.raw_os_error() == Some(Errno::ENOTEMPTY as i32)
                {
                    Ok(())
                } else {
                    Err(Error::Io(err))
                }
            }
        }
    }

    fn make_directory(&self, dirname: &Path) -> Result<()> {
        let path = self.pathname.join(dirname);

        if path.is_file() {
            fs::remove_file(&path)?;
        }
        if !path.is_dir() {
            fs::create_dir(&path)?;
        }

        Ok(())
    }
}
