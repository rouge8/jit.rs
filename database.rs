use crate::object::Object;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug)]
pub struct Database {
    pathname: PathBuf,
}

impl Database {
    pub fn new(pathname: PathBuf) -> Self {
        Database { pathname }
    }

    pub fn store<T>(&self, object: &T)
    where
        T: Object,
    {
        &self.write_object(object.oid(), object.content());
    }

    fn write_object(&self, oid: String, content: Vec<u8>) {
        let object_path = &self.pathname.join(&oid[0..2]).join(&oid[2..]);

        if object_path.exists() {
            return ();
        }

        let dirname = object_path.parent().unwrap();
        let temp_path = dirname.join(Uuid::new_v4().to_simple().to_string());

        // TODO: Only create `dirname` if it doesn't already exist
        fs::create_dir_all(&dirname).unwrap();

        {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)
                .unwrap();

            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
            encoder.write_all(&content).unwrap();

            let compressed = encoder.finish().unwrap();
            file.write_all(&compressed).unwrap();
        }

        fs::rename(&temp_path, &object_path).unwrap();
    }
}
