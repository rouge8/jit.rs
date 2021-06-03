use crate::database::object::Object;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::fs;
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
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
}

impl Database {
    pub fn new(pathname: PathBuf) -> Self {
        Database { pathname }
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

    fn write_object(&self, oid: String, content: Vec<u8>) -> io::Result<()> {
        let object_path = &self.pathname.join(&oid[0..2]).join(&oid[2..]);

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
}
