use crate::database::entry::Entry as DatabaseEntry;
use crate::errors::{Error, Result};
use crate::lockfile::Lockfile;
use crate::util::is_executable;
use crate::util::parent_directories;
use crate::util::path_to_string;
use hex::ToHex;
use sha1::{Digest, Sha1};
use std::cmp::min;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::convert::TryInto;
use std::fs;
use std::fs::File;
use std::io;
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::str;

const MAX_PATH_SIZE: u16 = 0xfff;
const CHECKSUM_SIZE: usize = 20;
const HEADER_SIZE: usize = 12;

#[derive(Debug)]
pub struct Index {
    pathname: PathBuf,
    pub entries: BTreeMap<(String, u16), Entry>,
    parents: HashMap<String, HashSet<String>>,
    lockfile: Lockfile,
    changed: bool,
}

impl Index {
    pub fn new(pathname: PathBuf) -> Self {
        Index {
            pathname: pathname.clone(),
            entries: BTreeMap::new(),
            parents: HashMap::new(),
            lockfile: Lockfile::new(pathname),
            changed: false,
        }
    }

    pub fn add(&mut self, pathname: PathBuf, oid: String, stat: fs::Metadata) {
        let pathname = path_to_string(&pathname);
        for stage in 1..=3 {
            self.remove_entry_with_stage(&pathname, stage);
        }

        let entry = Entry::new(&pathname, oid, stat);
        self.discard_conflicts(&entry);
        self.store_entry(entry);
        self.changed = true;
    }

    pub fn write_updates(&mut self) -> Result<()> {
        if !self.changed {
            self.lockfile.rollback()?;
            return Ok(());
        }

        let mut writer = Checksum::new(&self.lockfile);

        // Header
        let mut header: Vec<u8> = vec![];
        header.extend_from_slice(b"DIRC");
        header.extend_from_slice(&2u32.to_be_bytes()); // version number
        header.extend_from_slice(&(self.entries.len() as u32).to_be_bytes());
        writer.write(&header)?;

        // Entries
        for entry in self.entries.values() {
            writer.write(&entry.bytes())?;
        }

        writer.write_checksum()?;
        self.lockfile.commit()?;

        self.changed = false;

        Ok(())
    }

    pub fn load_for_update(&mut self) -> Result<()> {
        self.lockfile.hold_for_update()?;
        self.load()?;

        Ok(())
    }

    pub fn load(&mut self) -> Result<()> {
        self.clear();

        if let Some(file) = self.open_index_file()? {
            let mut reader = Checksum::new(file);
            let count = self.read_header(&mut reader)?;
            self.read_entries(&mut reader, count)?;
            reader.verify_checksum()?;
        }

        Ok(())
    }

    pub fn release_lock(&mut self) -> Result<()> {
        self.lockfile.rollback()?;

        Ok(())
    }

    pub fn tracked_file(&self, path: &Path) -> bool {
        (0..=3).any(|stage| {
            let key = (path_to_string(path), stage);
            self.entries.contains_key(&key)
        })
    }

    pub fn tracked(&self, path: &Path) -> bool {
        let key = path_to_string(path);
        self.tracked_file(path) || self.parents.contains_key(&key)
    }

    pub fn add_conflict_set(&mut self, pathname: &str, items: Vec<Option<DatabaseEntry>>) {
        assert_eq!(items.len(), 3);

        self.remove_entry_with_stage(pathname, 0);

        for (n, item) in items.iter().enumerate() {
            if let Some(item) = item {
                let entry = Entry::create_from_db(pathname, item, n + 1);
                self.store_entry(entry);
            }
        }
        self.changed = true;
    }

    pub fn update_entry_stat(&mut self, entry: &mut Entry, stat: &fs::Metadata) {
        entry.update_stat(stat);
        self.changed = true;
    }

    pub fn has_conflict(&self) -> bool {
        self.entries.values().any(|entry| entry.stage() > 0)
    }

    /// Arguments:
    ///
    /// * `path`: The path.
    /// * `stage`: The index stage, from `0..=3`.
    pub fn entry_for_path(&self, path: &str, stage: u16) -> Option<&Entry> {
        self.entries.get(&(path.to_string(), stage))
    }

    pub fn remove(&mut self, pathname: &Path) {
        let pathname = path_to_string(pathname);
        self.remove_entry(&pathname);
        self.remove_children(&pathname);
        self.changed = true;
    }

    fn clear(&mut self) {
        self.entries = BTreeMap::new();
        self.parents = HashMap::new();
        self.changed = false;
    }

    fn open_index_file(&self) -> Result<Option<File>> {
        let f = File::open(&self.pathname);

        match f {
            Ok(file) => Ok(Some(file)),
            Err(error) => match error.kind() {
                io::ErrorKind::NotFound => Ok(None),
                _ => Err(error.into()),
            },
        }
    }

    fn read_header(&self, reader: &mut Checksum<File>) -> Result<u32> {
        let data = reader.read(HEADER_SIZE)?;
        let signature = str::from_utf8(&data[0..4])?;
        let version = u32::from_be_bytes(data[4..8].try_into()?);
        let count = u32::from_be_bytes(data[8..12].try_into()?);

        if signature != "DIRC" {
            return Err(Error::InvalidSignature {
                expected: String::from("DIRC"),
                got: signature.to_string(),
            });
        }
        if version != 2 {
            return Err(Error::InvalidVersion {
                expected: 2,
                got: version,
            });
        }

        Ok(count)
    }

    fn read_entries(&mut self, reader: &mut Checksum<File>, count: u32) -> Result<()> {
        for _i in 0..count {
            let mut entry = reader.read(64)?;

            while entry.last().unwrap() != &0u8 {
                entry.extend_from_slice(&reader.read(8)?)
            }

            self.store_entry(Entry::parse(&entry)?);
        }

        Ok(())
    }

    fn store_entry(&mut self, entry: Entry) {
        for parent in entry.parent_directories() {
            let parent = path_to_string(&parent);

            if let Some(children) = self.parents.get_mut(&parent) {
                children.insert(entry.path.clone());
            } else {
                let mut children = HashSet::new();
                children.insert(entry.path.clone());
                self.parents.insert(parent, children);
            }
        }

        self.entries.insert(entry.key(), entry);
    }

    fn discard_conflicts(&mut self, entry: &Entry) {
        for parent in entry.parent_directories() {
            let parent = path_to_string(&parent);
            self.remove_entry(&parent);
        }
        self.remove_children(&entry.path);
    }

    fn remove_children(&mut self, path: &str) {
        let mut to_remove = vec![];

        if let Some(children) = self.parents.get(path) {
            for child in children.iter() {
                to_remove.push(child.clone());
            }
        }

        for child in to_remove {
            self.remove_entry(&child);
        }
    }

    fn remove_entry(&mut self, pathname: &str) {
        for stage in 0..=3 {
            self.remove_entry_with_stage(pathname, stage);
        }
    }

    fn remove_entry_with_stage(&mut self, pathname: &str, stage: u16) {
        if let Some(entry) = self.entries.remove(&(pathname.to_string(), stage)) {
            for dirname in entry.parent_directories() {
                let dirname = path_to_string(&dirname);

                if let Some(children) = self.parents.get_mut(&dirname) {
                    children.remove(pathname);
                    if children.is_empty() {
                        self.parents.remove(&dirname);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    ctime: i64,
    ctime_nsec: i64,
    // `mtime` and `mtime_nsec` are public so they can be inspected in `status_test.rs`
    pub mtime: i64,
    pub mtime_nsec: i64,
    dev: u64,
    ino: u64,
    pub mode: u32,
    uid: u32,
    gid: u32,
    size: u64,
    pub oid: String,
    flags: u16,
    pub path: String,
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
            mode: Entry::mode_for_stat(&stat),
            uid: stat.uid(),
            gid: stat.gid(),
            size: stat.size(),
            oid,
            flags: min(pathname.len() as u16, MAX_PATH_SIZE),
            path: pathname.to_string(),
        }
    }

    pub fn create_from_db(pathname: &str, item: &DatabaseEntry, n: usize) -> Self {
        let flags = ((n as u16) << 12) | min(pathname.len() as u16, MAX_PATH_SIZE);

        Self {
            ctime: 0,
            ctime_nsec: 0,
            mtime: 0,
            mtime_nsec: 0,
            dev: 0,
            ino: 0,
            mode: item.mode,
            uid: 0,
            gid: 0,
            size: 0,
            oid: item.oid.clone(),
            flags,
            path: pathname.to_string(),
        }
    }

    pub fn mode_for_stat(stat: &fs::Metadata) -> u32 {
        if is_executable(stat.mode()) {
            0o100755u32
        } else {
            0o100644u32
        }
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut metadata: Vec<u32> = Vec::with_capacity(10);

        for i in 0..10 {
            metadata.push(u32::from_be_bytes(data[i * 4..(i + 1) * 4].try_into()?));
        }

        let oid = data[40..60].to_vec().encode_hex::<String>();
        let flags = u16::from_be_bytes(data[60..62].try_into()?);
        let path = str::from_utf8(&data[62..])?
            .trim_end_matches('\0')
            .to_string();

        Ok(Entry {
            ctime: i64::from(metadata[0]),
            ctime_nsec: i64::from(metadata[1]),
            mtime: i64::from(metadata[2]),
            mtime_nsec: i64::from(metadata[3]),
            dev: u64::from(metadata[4]),
            ino: u64::from(metadata[5]),
            mode: metadata[6],
            uid: metadata[7],
            gid: metadata[8],
            size: u64::from(metadata[9]),
            oid,
            flags,
            path,
        })
    }

    fn key(&self) -> (String, u16) {
        (self.path.clone(), self.stage())
    }

    pub fn stage(&self) -> u16 {
        (self.flags >> 12) & 0x3
    }

    fn parent_directories(&self) -> Vec<PathBuf> {
        parent_directories(PathBuf::from(&self.path))
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

    pub fn stat_match(&self, stat: &fs::Metadata) -> bool {
        (self.mode == Entry::mode_for_stat(stat)) && (self.size == 0 || self.size == stat.size())
    }

    pub fn times_match(&self, stat: &fs::Metadata) -> bool {
        (self.ctime == stat.ctime())
            && (self.ctime_nsec == stat.ctime_nsec())
            && (self.mtime == stat.mtime())
            && (self.mtime_nsec == stat.mtime_nsec())
    }

    fn update_stat(&mut self, stat: &fs::Metadata) {
        self.ctime = stat.ctime();
        self.ctime_nsec = stat.ctime_nsec();
        self.mtime = stat.mtime();
        self.mtime_nsec = stat.mtime_nsec();
        self.dev = stat.dev();
        self.ino = stat.ino();
        self.mode = Entry::mode_for_stat(stat);
        self.uid = stat.uid();
        self.gid = stat.gid();
        self.size = stat.size();
    }
}

#[derive(Debug)]
struct Checksum<T>
where
    T: Read + Write,
{
    file: T,
    digest: Sha1,
}

impl<T> Checksum<T>
where
    T: Read + Write,
{
    fn new(file: T) -> Self {
        Checksum {
            file,
            digest: Sha1::new(),
        }
    }

    fn read(&mut self, size: usize) -> Result<Vec<u8>> {
        let mut data = vec![0; size];
        self.file.read_exact(&mut data)?;
        self.digest.update(&data);

        Ok(data)
    }

    fn verify_checksum(&mut self) -> Result<()> {
        let mut sum = vec![0; CHECKSUM_SIZE];
        self.file.read_exact(&mut sum)?;

        let expected = self.digest.clone().finalize().to_vec();
        if sum != expected {
            return Err(Error::InvalidChecksum);
        }

        Ok(())
    }

    fn write(&mut self, data: &[u8]) -> Result<()> {
        self.file.write_all(data)?;
        self.digest.update(data);

        Ok(())
    }

    fn write_checksum(&mut self) -> Result<()> {
        self.file
            .write_all(&self.digest.clone().finalize().to_vec())?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::tests::random_oid;
    use tempfile::TempDir;

    // Release the lock when dropping an `Index`, but only in tests
    impl Drop for Index {
        fn drop(&mut self) {
            let _ = self.lockfile.rollback();
        }
    }

    #[test]
    fn add_a_single_file() -> Result<()> {
        let tmp_dir = TempDir::new()?;
        let mut index = Index::new(tmp_dir.path().join("index"));

        let stat = fs::metadata(&tmp_dir)?;
        let oid = random_oid();

        index.add(PathBuf::from("alice.txt"), oid, stat);

        assert_eq!(
            index.entries.keys().cloned().collect::<Vec<_>>(),
            vec![(String::from("alice.txt"), 0)],
        );

        Ok(())
    }

    #[test]
    fn replace_a_file_with_a_directory() -> Result<()> {
        let tmp_dir = TempDir::new()?;
        let mut index = Index::new(tmp_dir.path().join("index"));

        let stat = fs::metadata(&tmp_dir)?;
        let oid = random_oid();

        index.add(PathBuf::from("alice.txt"), oid.clone(), stat.clone());
        index.add(PathBuf::from("bob.txt"), oid.clone(), stat.clone());

        index.add(PathBuf::from("alice.txt/nested"), oid, stat);

        assert_eq!(
            index.entries.keys().cloned().collect::<Vec<_>>(),
            vec![
                (String::from("alice.txt/nested"), 0),
                (String::from("bob.txt"), 0)
            ],
        );

        Ok(())
    }

    #[test]
    fn replace_a_directory_with_a_file() -> Result<()> {
        let tmp_dir = TempDir::new()?;
        let mut index = Index::new(tmp_dir.path().join("index"));

        let stat = fs::metadata(&tmp_dir)?;
        let oid = random_oid();

        index.add(PathBuf::from("alice.txt"), oid.clone(), stat.clone());
        index.add(PathBuf::from("nested/bob.txt"), oid.clone(), stat.clone());

        index.add(PathBuf::from("nested"), oid, stat);

        assert_eq!(
            index.entries.keys().cloned().collect::<Vec<_>>(),
            vec![(String::from("alice.txt"), 0), (String::from("nested"), 0)],
        );

        Ok(())
    }

    #[test]
    fn recursively_replace_a_directory_with_a_file() -> Result<()> {
        let tmp_dir = TempDir::new()?;
        let mut index = Index::new(tmp_dir.path().join("index"));

        let stat = fs::metadata(&tmp_dir)?;
        let oid = random_oid();

        index.add(PathBuf::from("alice.txt"), oid.clone(), stat.clone());
        index.add(PathBuf::from("nested/bob.txt"), oid.clone(), stat.clone());
        index.add(
            PathBuf::from("nested/inner/claire.txt"),
            oid.clone(),
            stat.clone(),
        );

        index.add(PathBuf::from("nested"), oid, stat);

        assert_eq!(
            index.entries.keys().cloned().collect::<Vec<_>>(),
            vec![(String::from("alice.txt"), 0), (String::from("nested"), 0)],
        );

        Ok(())
    }
}
