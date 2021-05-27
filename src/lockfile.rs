use crate::errors::{Error, Result};
use std::fs;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::{Read, Write};
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
                .open(&self.lock_path)?;

            self.lock = Some(open_file);
        }

        Ok(())
    }

    pub fn write(&self, bytes: &[u8]) -> Result<()> {
        self.err_on_stale_lock()?;

        let mut lock = self.lock.as_ref().unwrap();

        lock.write_all(bytes)?;

        Ok(())
    }

    pub fn commit(&mut self) -> Result<()> {
        self.err_on_stale_lock()?;

        self.lock = None;
        fs::rename(&self.lock_path, &self.file_path)?;

        Ok(())
    }

    pub fn rollback(&mut self) -> Result<()> {
        self.err_on_stale_lock()?;

        fs::remove_file(&self.lock_path)?;
        self.lock = None;

        Ok(())
    }

    fn err_on_stale_lock(&self) -> io::Result<()> {
        if self.lock.is_none() {
            Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Not holding lock on file: {:?}", self.lock_path),
            ))
        } else {
            Ok(())
        }
    }
}

impl Read for Lockfile {
    fn read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
        self.err_on_stale_lock()?;

        let mut lock = self.lock.as_ref().unwrap();
        lock.read(&mut buf)
    }
}

impl Write for Lockfile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.err_on_stale_lock()?;

        let mut lock = self.lock.as_ref().unwrap();
        lock.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.err_on_stale_lock()?;

        let mut lock = self.lock.as_ref().unwrap();
        lock.flush()
    }
}

impl<'a> Read for &'a Lockfile {
    fn read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
        self.err_on_stale_lock()?;

        let mut lock = self.lock.as_ref().unwrap();
        lock.read(&mut buf)
    }
}

impl<'a> Write for &'a Lockfile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.err_on_stale_lock()?;

        let mut lock = self.lock.as_ref().unwrap();
        lock.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.err_on_stale_lock()?;

        let mut lock = self.lock.as_ref().unwrap();
        lock.flush()
    }
}
