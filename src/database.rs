use crate::database::blob::Blob;
use crate::database::commit::Commit;
use crate::database::entry::Entry;
use crate::database::object::Object;
use crate::database::tree::{Tree, TreeEntry, TREE_MODE};
use crate::database::tree_diff::{Differ, TreeDiff, TreeDiffChanges};
use crate::errors::Result;
use crate::path_filter::PathFilter;
use crate::util::path_to_string;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use itertools::Itertools;
use std::collections::HashMap;
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

    /// Load an object ID, returning a `ParsedObject`.
    pub fn load(&self, oid: &str) -> io::Result<ParsedObject> {
        // TODO: Cache this in self.objects
        self.read_object(oid)
    }

    /// Load a commit by its object ID, returning a `Commit`.
    pub fn load_commit(&self, oid: &str) -> io::Result<Commit> {
        match self.load(oid)? {
            ParsedObject::Commit(commit) => Ok(commit),
            _ => unreachable!(),
        }
    }

    /// Load a blob by its object ID, returning a `Blob`.
    pub fn load_blob(&self, oid: &str) -> io::Result<Blob> {
        match self.load(oid)? {
            ParsedObject::Blob(blob) => Ok(blob),
            _ => unreachable!(),
        }
    }

    /// Load a tree by its object ID, returning a `Tree`.
    pub fn load_tree(&self, oid: &str) -> io::Result<Tree> {
        match self.load(oid)? {
            ParsedObject::Tree(tree) => Ok(tree),
            _ => unreachable!(),
        }
    }

    pub fn load_tree_entry(
        &self,
        oid: &str,
        pathname: Option<&Path>,
    ) -> io::Result<Option<TreeEntry>> {
        let commit = self.load_commit(oid)?;
        let root = Entry::new(commit.tree, TREE_MODE);

        let mut entry = Some(TreeEntry::Entry(root));
        if pathname.is_none() {
            return Ok(entry);
        }

        for name in pathname.unwrap().iter() {
            let name = PathBuf::from(name);

            entry = if let Some(entry) = entry {
                self.load_tree(&entry.oid())?
                    .entries
                    .get(&name)
                    .map(|entry| entry.to_owned())
            } else {
                None
            };
        }

        Ok(entry)
    }

    pub fn load_tree_list(
        &self,
        oid: Option<&str>,
        pathname: Option<&Path>,
    ) -> io::Result<HashMap<String, TreeEntry>> {
        let mut list = HashMap::new();

        if let Some(oid) = oid {
            let entry = self.load_tree_entry(oid, pathname)?;
            self.build_list(&mut list, entry, pathname.unwrap_or_else(|| Path::new("")))?;
        }

        Ok(list)
    }

    fn build_list(
        &self,
        list: &mut HashMap<String, TreeEntry>,
        entry: Option<TreeEntry>,
        prefix: &Path,
    ) -> io::Result<()> {
        if let Some(entry) = entry {
            if !entry.is_tree() {
                list.insert(path_to_string(prefix), entry);
                return Ok(());
            }

            for (name, item) in self.load_tree(&entry.oid())?.entries {
                self.build_list(list, Some(item), &prefix.join(name))?;
            }
        }

        Ok(())
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

    fn read_object(&self, oid: &str) -> io::Result<ParsedObject> {
        let compressed_data = fs::read(self.object_path(oid))?;
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
            "blob" => Ok(Blob::parse(rest, oid)),
            "tree" => Ok(Tree::parse(rest)),
            "commit" => Ok(Commit::parse(rest, oid)),
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

impl Differ for Database {
    fn tree_diff(
        &self,
        a: Option<&str>,
        b: Option<&str>,
        filter: Option<&PathFilter>,
    ) -> Result<TreeDiffChanges> {
        let empty_filter = PathFilter::new(None, None);

        let filter = if let Some(filter) = filter {
            filter
        } else {
            &empty_filter
        };
        let mut diff = TreeDiff::new(self);
        diff.compare_oids(a, b, filter)?;
        Ok(diff.changes)
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
        use crate::database::tree::TreeEntry;
        use indexmap::IndexMap;
        use rstest::{fixture, rstest};
        use std::collections::{BTreeMap, HashMap};
        use std::path::PathBuf;
        use tempfile::TempDir;

        fn store_tree(database: &Database, contents: HashMap<&str, &str>) -> String {
            let mut entries = BTreeMap::new();
            for (path, data) in contents {
                let blob = Blob::new(data.as_bytes().to_vec());
                database.store(&blob).unwrap();

                entries.insert(
                    PathBuf::from(path),
                    TreeEntry::Entry(Entry::new(blob.oid(), 0o100644)),
                );
            }

            let tree = Tree::new(Some(entries));
            tree.traverse(&|t| database.store(t).unwrap());

            tree.oid()
        }

        #[fixture]
        fn database() -> Database {
            Database::new(TempDir::new().unwrap().path().to_path_buf())
        }

        #[rstest]
        fn report_a_changed_file(database: Database) -> Result<()> {
            let tree_a_contents = HashMap::from([("alice.txt", "alice"), ("bob.txt", "bob")]);
            let tree_a = store_tree(&database, tree_a_contents);

            let tree_b_contents = HashMap::from([("alice.txt", "changed"), ("bob.txt", "bob")]);
            let tree_b = store_tree(&database, tree_b_contents);

            let expected = IndexMap::from([(
                PathBuf::from("alice.txt"),
                (
                    Some(Entry::new(
                        String::from("ca56b59dbf8c0884b1b9ceb306873b24b73de969"),
                        0o100644,
                    )),
                    Some(Entry::new(
                        String::from("21fb1eca31e64cd3914025058b21992ab76edcf9"),
                        0o100644,
                    )),
                ),
            )]);

            assert_eq!(
                database.tree_diff(Some(&tree_a), Some(&tree_b), None)?,
                expected
            );

            Ok(())
        }

        #[rstest]
        fn report_an_added_file(database: Database) -> Result<()> {
            let tree_a_contents = HashMap::from([("alice.txt", "alice")]);
            let tree_a = store_tree(&database, tree_a_contents);

            let tree_b_contents = HashMap::from([("alice.txt", "alice"), ("bob.txt", "bob")]);
            let tree_b = store_tree(&database, tree_b_contents);

            let expected = IndexMap::from([(
                PathBuf::from("bob.txt"),
                (
                    None,
                    Some(Entry::new(
                        String::from("2529de8969e5ee206e572ed72a0389c3115ad95c"),
                        0o100644,
                    )),
                ),
            )]);

            assert_eq!(
                database.tree_diff(Some(&tree_a), Some(&tree_b), None)?,
                expected
            );

            Ok(())
        }

        #[rstest]
        fn report_a_deleted_file(database: Database) -> Result<()> {
            let tree_a_contents = HashMap::from([("alice.txt", "alice"), ("bob.txt", "bob")]);
            let tree_a = store_tree(&database, tree_a_contents);

            let tree_b_contents = HashMap::from([("alice.txt", "alice")]);
            let tree_b = store_tree(&database, tree_b_contents);

            let expected = IndexMap::from([(
                PathBuf::from("bob.txt"),
                (
                    Some(Entry::new(
                        String::from("2529de8969e5ee206e572ed72a0389c3115ad95c"),
                        0o100644,
                    )),
                    None,
                ),
            )]);

            assert_eq!(
                database.tree_diff(Some(&tree_a), Some(&tree_b), None)?,
                expected
            );

            Ok(())
        }

        #[rstest]
        fn report_an_added_file_inside_a_directory(database: Database) -> Result<()> {
            let tree_a_contents = HashMap::from([("1.txt", "1"), ("outer/2.txt", "2")]);
            let tree_a = store_tree(&database, tree_a_contents);

            let tree_b_contents = HashMap::from([
                ("1.txt", "1"),
                ("outer/2.txt", "2"),
                ("outer/new/4.txt", "4"),
            ]);
            let tree_b = store_tree(&database, tree_b_contents);

            let expected = IndexMap::from([(
                PathBuf::from("outer/new/4.txt"),
                (
                    None,
                    Some(Entry::new(
                        String::from("bf0d87ab1b2b0ec1a11a3973d2845b42413d9767"),
                        0o100644,
                    )),
                ),
            )]);

            assert_eq!(
                database.tree_diff(Some(&tree_a), Some(&tree_b), None)?,
                expected
            );

            Ok(())
        }

        #[rstest]
        fn report_a_deleted_file_inside_a_directory(database: Database) -> Result<()> {
            let tree_a_contents = HashMap::from([
                ("1.txt", "1"),
                ("outer/2.txt", "2"),
                ("outer/inner/3.txt", "3"),
            ]);
            let tree_a = store_tree(&database, tree_a_contents);

            let tree_b_contents = HashMap::from([("1.txt", "1"), ("outer/2.txt", "2")]);
            let tree_b = store_tree(&database, tree_b_contents);

            let expected = IndexMap::from([(
                PathBuf::from("outer/inner/3.txt"),
                (
                    Some(Entry::new(
                        String::from("e440e5c842586965a7fb77deda2eca68612b1f53"),
                        0o100644,
                    )),
                    None,
                ),
            )]);

            assert_eq!(
                database.tree_diff(Some(&tree_a), Some(&tree_b), None)?,
                expected
            );

            Ok(())
        }
    }
}
