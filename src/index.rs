use crate::lockfile::Lockfile;
use crate::util::is_executable;
use anyhow::Result;
use sha1::{Digest, Sha1};
use std::cmp::min;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

const MAX_PATH_SIZE: u16 = 0xfff;

#[derive(Debug)]
pub struct Index {
    entries: BTreeMap<String, Entry>,
    lockfile: Lockfile,
}

impl Index {
    pub fn new(pathname: PathBuf) -> Self {
        Index {
            entries: BTreeMap::new(),
            lockfile: Lockfile::new(pathname),
        }
    }

    pub fn add(&mut self, pathname: &str, oid: String, stat: fs::Metadata) {
        let entry = Entry::new(pathname, oid, stat);
        self.entries.insert(pathname.to_string(), entry);
    }

    pub fn write_updates(&mut self) -> Result<()> {
        self.lockfile.hold_for_update()?;

        let mut bytes: Vec<u8> = vec![];

        // Header
        bytes.extend_from_slice(b"DIRC");
        bytes.extend_from_slice(&2u32.to_be_bytes()); // version number
        bytes.extend_from_slice(&(self.entries.len() as u32).to_be_bytes());

        // Entries
        for entry in self.entries.values() {
            bytes.append(&mut entry.bytes());
        }

        // SHA1 checksum
        let mut hash = Sha1::new().chain(&bytes).finalize().to_vec();
        bytes.append(&mut hash);

        self.lockfile.write(&bytes)?;
        self.lockfile.commit()
    }
}

#[derive(Debug)]
struct Entry {
    ctime: i64,
    ctime_nsec: i64,
    mtime: i64,
    mtime_nsec: i64,
    dev: u64,
    ino: u64,
    uid: u32,
    gid: u32,
    size: u64,
    flags: u16,
    mode: u32,
    oid: String,
    path: String,
}

impl Entry {
    fn new(pathname: &str, oid: String, stat: fs::Metadata) -> Self {
        Entry {
            ctime: stat.ctime(),
            ctime_nsec: stat.ctime_nsec(),
            mtime: stat.mtime(),
            mtime_nsec: stat.mtime_nsec(),
            dev: stat.dev(),
            ino: stat.ino(),
            mode: Entry::mode(stat.mode()),
            uid: stat.uid(),
            gid: stat.gid(),
            size: stat.size(),
            oid: oid.to_string(),
            flags: min(pathname.len() as u16, MAX_PATH_SIZE),
            path: pathname.to_string(),
        }
    }

    fn mode(mode: u32) -> u32 {
        if is_executable(mode) {
            0o100755u32
        } else {
            0o100644u32
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // 10 32-bit integers
        bytes.extend_from_slice(&(self.ctime as u32).to_be_bytes());
        bytes.extend_from_slice(&(self.ctime_nsec as u32).to_be_bytes());
        bytes.extend_from_slice(&(self.mtime as u32).to_be_bytes());
        bytes.extend_from_slice(&(self.mtime_nsec as u32).to_be_bytes());
        bytes.extend_from_slice(&(self.dev as u32).to_be_bytes());
        bytes.extend_from_slice(&(self.ino as u32).to_be_bytes());
        bytes.extend_from_slice(&(self.mode as u32).to_be_bytes());
        bytes.extend_from_slice(&(self.uid as u32).to_be_bytes());
        bytes.extend_from_slice(&(self.gid as u32).to_be_bytes());
        bytes.extend_from_slice(&(self.size as u32).to_be_bytes());

        // 20 bytes (40-char hex-string)
        bytes.extend_from_slice(&hex::decode(&self.oid).unwrap());

        // 16-bit
        bytes.extend_from_slice(&self.flags.to_be_bytes());

        bytes.extend_from_slice(self.path.as_bytes());
        bytes.push(0x0);

        // add padding
        while bytes.len() % 8 != 0 {
            bytes.push(0x0)
        }

        bytes
    }
}
