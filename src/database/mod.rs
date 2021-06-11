use crate::database::blob::Blob;
use crate::database::commit::Commit;
use crate::database::object::Object;
use crate::database::tree::Tree;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use itertools::Itertools;
use std::collections::HashMap;
use std::fs;
use std::fs::OpenOptions;
use std::io;
use std::io::{Read, Write};
use std::path::PathBuf;
use uuid::Uuid;

pub mod author;
pub mod blob;
pub mod commit;
pub mod entry;
pub mod object;
pub mod tree;

#[derive(Debug)]
pub struct Database {
    pathname: PathBuf,
    objects: HashMap<String, ParsedObject>,
}

impl Database {
    pub fn new(pathname: PathBuf) -> Self {
        Database {
            pathname,
            objects: HashMap::new(),
        }
    }

    pub fn store<T>(&self, object: &T) -> io::Result<()>
    where
        T: Object,
    {
        self.write_object(object.oid(), object.content())?;
        Ok(())
    }

    pub fn hash_object<T>(&self, object: &T) -> String
    where
        T: Object,
    {
        object.oid()
    }

    pub fn load(&mut self, oid: String) -> io::Result<&ParsedObject> {
        let object = self.read_object(&oid)?;

        self.objects.insert(oid.clone(), object);

        Ok(&self.objects[&oid])
    }

    pub fn short_oid(&self, oid: &str) -> String {
        oid[0..=6].to_string()
    }

    fn object_path(&self, oid: &str) -> PathBuf {
        self.pathname.join(&oid[0..2]).join(&oid[2..])
    }

    fn write_object(&self, oid: String, content: Vec<u8>) -> io::Result<()> {
        let object_path = self.object_path(&oid);

        if object_path.exists() {
            return Ok(());
        }

        let dirname = object_path.parent().unwrap();
        let temp_path = dirname.join(Uuid::new_v4().to_simple().to_string());

        // TODO: Only create `dirname` if it doesn't already exist
        fs::create_dir_all(&dirname)?;

        {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)?;

            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
            encoder.write_all(&content)?;

            let compressed = encoder.finish()?;
            file.write_all(&compressed)?;
        }

        fs::rename(&temp_path, &object_path)?;

        Ok(())
    }

    fn read_object(&self, oid: &str) -> io::Result<ParsedObject> {
        let compressed_data = fs::read(self.object_path(&oid))?;
        let mut data = vec![];
        let mut z = ZlibDecoder::new(&compressed_data[..]);
        z.read_to_end(&mut data)?;

        let (object_type, rest) = data
            .splitn(2, |c| *c as char == ' ')
            .collect_tuple()
            .unwrap();
        let object_type = std::str::from_utf8(object_type).expect("Invalid UTF-8");

        let (_size, rest) = rest
            .splitn(2, |c| *c as char == '\0')
            .collect_tuple()
            .unwrap();

        match object_type {
            "blob" => Ok(Blob::parse(rest)),
            "tree" => Ok(Tree::parse(rest)),
            "commit" => Ok(Commit::parse(rest)),
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
pub enum ParsedObject {
    Blob(Blob),
    Commit(Commit),
    Tree(Tree),
}
