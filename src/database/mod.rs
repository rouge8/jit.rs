use crate::database::blob::Blob;
use crate::database::commit::Commit;
use crate::database::object::Object;
use crate::database::tree::Tree;
use crate::database::tree_diff::{TreeDiff, TreeDiffChanges};
use crate::errors::Result;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use itertools::Itertools;
use std::fs;
use std::fs::OpenOptions;
use std::io;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub mod author;
pub mod blob;
pub mod commit;
pub mod entry;
pub mod object;
pub mod tree;
pub mod tree_diff;

#[derive(Debug)]
pub struct Database {
    pathname: PathBuf,
}

impl Database {
    pub fn new(pathname: PathBuf) -> Self {
        Database { pathname }
    }

    pub fn short_oid(oid: &str) -> String {
        oid[0..=6].to_string()
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

    pub fn load(&self, oid: &str) -> io::Result<ParsedObject> {
        // TODO: Cache this in self.objects
        self.read_object(oid)
    }

    pub fn prefix_match(&self, name: &str) -> io::Result<Vec<String>> {
        let path = self.object_path(name);
        let dirname = path.parent().unwrap();

        if !dirname.exists() {
            // No objects match the given name
            return Ok(vec![]);
        }

        let oids: Vec<_> = fs::read_dir(&dirname)?
            .map(|filename| {
                format!(
                    "{}{}",
                    dirname.file_name().unwrap().to_str().unwrap(),
                    filename.unwrap().file_name().to_str().unwrap()
                )
            })
            .filter(|oid| oid.starts_with(name))
            .collect();

        Ok(oids)
    }

    pub fn tree_diff(&self, a: &str, b: &str) -> Result<TreeDiffChanges> {
        let mut diff = TreeDiff::new(&self);
        diff.compare_oids(Some(a), Some(b), Path::new(""))?;
        Ok(diff.changes)
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
}

#[derive(Debug)]
pub enum ParsedObject {
    Blob(Blob),
    Commit(Commit),
    Tree(Tree),
}

impl ParsedObject {
    pub fn oid(&self) -> String {
        match self {
            ParsedObject::Blob(obj) => obj.oid(),
            ParsedObject::Commit(obj) => obj.oid(),
            ParsedObject::Tree(obj) => obj.oid(),
        }
    }

    pub fn r#type(&self) -> &str {
        match self {
            ParsedObject::Blob(obj) => obj.r#type(),
            ParsedObject::Commit(obj) => obj.r#type(),
            ParsedObject::Tree(obj) => obj.r#type(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod tree_diff {
        use super::*;
        use crate::database::entry::Entry;
        use rstest::{fixture, rstest};
        use std::collections::HashMap;
        use std::path::Path;
        use tempfile::TempDir;

        fn store_tree(database: &Database, contents: HashMap<&str, &str>) -> String {
            let entries: Vec<_> = contents
                .into_iter()
                .map(|(path, data)| {
                    let blob = Blob::new(data.as_bytes().to_vec());
                    database.store(&blob).unwrap();

                    Entry::new(Path::new(path), blob.oid(), 0o100644)
                })
                .collect();

            let tree = Tree::build(entries);
            tree.traverse(&|t| database.store(t).unwrap());

            tree.oid()
        }

        #[fixture]
        fn database() -> Database {
            Database::new(TempDir::new().unwrap().path().to_path_buf())
        }

        #[rstest]
        fn report_a_changed_file(database: Database) -> Result<()> {
            let mut tree_a_contents = HashMap::new();
            tree_a_contents.insert("alice.txt", "alice");
            tree_a_contents.insert("bob.txt", "bob");
            let tree_a = store_tree(&database, tree_a_contents);

            let mut tree_b_contents = HashMap::new();
            tree_b_contents.insert("alice.txt", "changed");
            tree_b_contents.insert("bob.txt", "bob");
            let tree_b = store_tree(&database, tree_b_contents);

            let mut expected = HashMap::new();
            expected.insert(
                PathBuf::from("alice.txt"),
                (
                    Some(Entry::new(
                        Path::new("alice.txt"),
                        String::from("ca56b59dbf8c0884b1b9ceb306873b24b73de969"),
                        0o100644,
                    )),
                    Some(Entry::new(
                        Path::new("alice.txt"),
                        String::from("21fb1eca31e64cd3914025058b21992ab76edcf9"),
                        0o100644,
                    )),
                ),
            );

            assert_eq!(database.tree_diff(&tree_a, &tree_b)?, expected);

            Ok(())
        }

        #[rstest]
        fn report_an_added_file(database: Database) -> Result<()> {
            let mut tree_a_contents = HashMap::new();
            tree_a_contents.insert("alice.txt", "alice");
            let tree_a = store_tree(&database, tree_a_contents);

            let mut tree_b_contents = HashMap::new();
            tree_b_contents.insert("alice.txt", "alice");
            tree_b_contents.insert("bob.txt", "bob");
            let tree_b = store_tree(&database, tree_b_contents);

            let mut expected = HashMap::new();
            expected.insert(
                PathBuf::from("bob.txt"),
                (
                    None,
                    Some(Entry::new(
                        Path::new("bob.txt"),
                        String::from("2529de8969e5ee206e572ed72a0389c3115ad95c"),
                        0o100644,
                    )),
                ),
            );

            assert_eq!(database.tree_diff(&tree_a, &tree_b)?, expected);

            Ok(())
        }

        #[rstest]
        fn report_a_deleted_file(database: Database) -> Result<()> {
            let mut tree_a_contents = HashMap::new();
            tree_a_contents.insert("alice.txt", "alice");
            tree_a_contents.insert("bob.txt", "bob");
            let tree_a = store_tree(&database, tree_a_contents);

            let mut tree_b_contents = HashMap::new();
            tree_b_contents.insert("alice.txt", "alice");
            let tree_b = store_tree(&database, tree_b_contents);

            let mut expected = HashMap::new();
            expected.insert(
                PathBuf::from("bob.txt"),
                (
                    Some(Entry::new(
                        Path::new("bob.txt"),
                        String::from("2529de8969e5ee206e572ed72a0389c3115ad95c"),
                        0o100644,
                    )),
                    None,
                ),
            );

            assert_eq!(database.tree_diff(&tree_a, &tree_b)?, expected);

            Ok(())
        }

        #[rstest]
        fn report_an_added_file_inside_a_directory(database: Database) -> Result<()> {
            let mut tree_a_contents = HashMap::new();
            tree_a_contents.insert("1.txt", "1");
            tree_a_contents.insert("outer/2.txt", "2");
            let tree_a = store_tree(&database, tree_a_contents);

            let mut tree_b_contents = HashMap::new();
            tree_b_contents.insert("1.txt", "1");
            tree_b_contents.insert("outer/2.txt", "2");
            tree_b_contents.insert("outer/new/4.txt", "4");
            let tree_b = store_tree(&database, tree_b_contents);

            let mut expected = HashMap::new();
            expected.insert(
                PathBuf::from("outer/new/4.txt"),
                (
                    None,
                    Some(Entry::new(
                        Path::new("4.txt"),
                        String::from("bf0d87ab1b2b0ec1a11a3973d2845b42413d9767"),
                        0o100644,
                    )),
                ),
            );

            assert_eq!(database.tree_diff(&tree_a, &tree_b)?, expected);

            Ok(())
        }

        #[rstest]
        fn report_a_deleted_file_inside_a_directory(database: Database) -> Result<()> {
            let mut tree_a_contents = HashMap::new();
            tree_a_contents.insert("1.txt", "1");
            tree_a_contents.insert("outer/2.txt", "2");
            tree_a_contents.insert("outer/inner/3.txt", "3");
            let tree_a = store_tree(&database, tree_a_contents);

            let mut tree_b_contents = HashMap::new();
            tree_b_contents.insert("1.txt", "1");
            tree_b_contents.insert("outer/2.txt", "2");
            let tree_b = store_tree(&database, tree_b_contents);

            let mut expected = HashMap::new();
            expected.insert(
                PathBuf::from("outer/inner/3.txt"),
                (
                    Some(Entry::new(
                        Path::new("3.txt"),
                        String::from("e440e5c842586965a7fb77deda2eca68612b1f53"),
                        0o100644,
                    )),
                    None,
                ),
            );

            assert_eq!(database.tree_diff(&tree_a, &tree_b)?, expected);

            Ok(())
        }
    }
}
