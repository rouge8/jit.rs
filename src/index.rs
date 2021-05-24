use crate::lockfile::Lockfile;
use crate::util::basename;
use crate::util::is_executable;
use crate::util::parent_directories;
use anyhow::{bail, Result};
use hex::ToHex;
use sha1::{Digest, Sha1};
use std::cmp::min;
use std::collections::BTreeMap;
use std::convert::TryInto;
use std::fs;
use std::fs::File;
use std::io;
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::str;

const MAX_PATH_SIZE: u16 = 0xfff;
const CHECKSUM_SIZE: usize = 20;
const HEADER_SIZE: usize = 12;

#[derive(Debug)]
pub struct Index {
    pub entries: BTreeMap<String, Entry>,
    pathname: PathBuf,
    lockfile: Lockfile,
    changed: bool,
}

impl Index {
    pub fn new(pathname: PathBuf) -> Self {
        Index {
            entries: BTreeMap::new(),
            pathname: pathname.clone(),
            lockfile: Lockfile::new(pathname),
            changed: false,
        }
    }

    pub fn add(&mut self, pathname: PathBuf, oid: String, stat: fs::Metadata) {
        let pathname = pathname.to_str().unwrap();
        let entry = Entry::new(pathname, oid, stat);
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

    fn clear(&mut self) {
        self.entries = BTreeMap::new();
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
            bail!("Signature: expected 'DIRC' but found '{}'", signature);
        }
        if version != 2 {
            bail!("Version: expected '2' but found '{}'", version);
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
        self.entries.insert(entry.path.clone(), entry);
    }

    fn discard_conflicts(&mut self, entry: &Entry) {
        for parent in entry.parent_directories() {
            let parent = parent.to_str().unwrap();
            self.entries.remove(parent);
        }
    }
}

#[derive(Debug)]
pub struct Entry {
    ctime: i64,
    ctime_nsec: i64,
    mtime: i64,
    mtime_nsec: i64,
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
            mode: Entry::mode(stat.mode()),
            uid: stat.uid(),
            gid: stat.gid(),
            size: stat.size(),
            oid,
            flags: min(pathname.len() as u16, MAX_PATH_SIZE),
            path: pathname.to_string(),
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

    fn mode(mode: u32) -> u32 {
        if is_executable(mode) {
            0o100755u32
        } else {
            0o100644u32
        }
    }

    fn basename(&self) -> PathBuf {
        basename(PathBuf::from(&self.path))
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
            bail!("Checksum does not match value stored on disk");
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
    use crate::util::random_oid;
    use tempfile::TempDir;

    // Release the lock when dropping an `Index`, but only in tests
    impl Drop for Index {
        fn drop(&mut self) {
            let _ = self.lockfile.rollback();
        }
    }

    #[test]
    fn load_for_update_adds_files_to_index_entries() -> Result<()> {
        let root_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let git_path = root_path.join(".git");

        let mut index = Index::new(git_path.join("index"));
        index.load_for_update()?;

        assert!(index.entries.get("src/main.rs").is_some());
        assert!(index.entries.get("src/lockfile.rs").is_some());

        Ok(())
    }

    #[test]
    fn add_a_single_file() -> Result<()> {
        let tmp_dir = TempDir::new()?;
        let mut index = Index::new(tmp_dir.path().join("index"));

        let stat = fs::metadata(&tmp_dir)?;
        let oid = random_oid();

        index.add(PathBuf::from("alice.txt"), oid, stat);

        assert_eq!(
            index.entries.keys().cloned().collect::<Vec<String>>(),
            vec![String::from("alice.txt")],
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
            index.entries.keys().cloned().collect::<Vec<String>>(),
            vec![String::from("alice.txt/nested"), String::from("bob.txt")],
        );

        Ok(())
    }
}
