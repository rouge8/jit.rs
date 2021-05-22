use std::fs;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::Write;
use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, LockfileErr>;

#[derive(Error, Debug)]
pub enum LockfileErr {
    #[error("{lock_path}: {err}")]
    IO { lock_path: PathBuf, err: io::Error },
    #[error("Not holding lock on file: {0}")]
    StaleLock(PathBuf),
}

#[derive(Debug)]
pub struct Lockfile {
    file_path: PathBuf,
    lock_path: PathBuf,
    lock: Option<File>,
}

impl Lockfile {
    pub fn new(path: PathBuf) -> Self {
        let lock_path = path.with_extension("lock");

        Lockfile {
            file_path: path,
            lock_path,
            lock: None,
        }
    }

    pub fn hold_for_update(&mut self) -> Result<()> {
        if self.lock.is_none() {
            let open_file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&self.lock_path);

            let open_file = match open_file {
                Ok(file) => file,
                Err(err) => return Err(self.io_error(err)),
            };

            self.lock = Some(open_file);
        }

        Ok(())
    }

    pub fn write(&self, contents: &str) -> Result<()> {
        self.err_on_stale_lock()?;

        let mut lock = self.lock.as_ref().unwrap();

        match lock.write_all(contents.as_bytes()) {
            Ok(()) => Ok(()),
            Err(err) => Err(self.io_error(err)),
        }
    }

    pub fn commit(&mut self) -> Result<()> {
        self.err_on_stale_lock()?;

        self.lock = None;
        match fs::rename(&self.lock_path, &self.file_path) {
            Ok(()) => Ok(()),
            Err(err) => Err(self.io_error(err)),
        }
    }

    fn err_on_stale_lock(&self) -> Result<()> {
        if self.lock.is_none() {
            Err(LockfileErr::StaleLock(self.lock_path.clone()))
        } else {
            Ok(())
        }
    }

    pub fn io_error(&self, err: io::Error) -> LockfileErr {
        LockfileErr::IO {
            lock_path: self.lock_path.clone(),
            err,
        }
    }
}
