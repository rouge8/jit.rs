use anyhow::{bail, Context, Result};
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

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
        // TODO: Handle file already exists
        if self.lock.is_none() {
            let open_file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&self.lock_path)
                .with_context(|| format!("{:?}", self.lock_path))?;

            self.lock = Some(open_file);
        }

        Ok(())
    }

    pub fn write(&self, bytes: &[u8]) -> Result<()> {
        self.err_on_stale_lock()?;

        let mut lock = self.lock.as_ref().unwrap();

        lock.write_all(bytes)
            .with_context(|| format!("{:?}", self.lock_path))?;

        Ok(())
    }

    pub fn commit(&mut self) -> Result<()> {
        self.err_on_stale_lock()?;

        self.lock = None;
        fs::rename(&self.lock_path, &self.file_path)
            .with_context(|| format!("{:?}", self.lock_path))?;

        Ok(())
    }

    fn err_on_stale_lock(&self) -> Result<()> {
        if self.lock.is_none() {
            bail!("Not holding lock on file: {:?}", self.lock_path);
        } else {
            Ok(())
        }
    }
}
