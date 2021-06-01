use std::path::{Path, PathBuf};

pub fn is_executable(mode: u32) -> bool {
    mode & 0o1111 != 0
}

pub fn parent_directories(mut path: PathBuf) -> Vec<PathBuf> {
    let mut parents = Vec::new();

    // TODO: path.ancestors()
    while let Some(parent) = path.parent() {
        let parent = parent.to_path_buf();
        path = parent.clone();

        if parent != PathBuf::from("") {
            parents.insert(0, parent);
        }
    }

    parents
}

pub fn basename(path: PathBuf) -> PathBuf {
    PathBuf::from(PathBuf::from(&path).file_name().unwrap())
}

pub fn path_to_string(path: &Path) -> String {
    path.to_str().unwrap().to_string()
}

#[cfg(test)]
pub mod tests {
    use crate::commands::execute;
    use crate::errors::Result;
    use crate::repository::Repository;
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};
    use sha1::{Digest, Sha1};
    use std::collections::{HashMap, VecDeque};
    use std::fs;
    use std::fs::OpenOptions;
    use std::io::{Cursor, Write};
    use std::path::PathBuf;
    use tempfile::TempDir;

    pub fn random_oid() -> String {
        let rand_string: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();
        let hash = Sha1::new().chain(rand_string).finalize();

        format!("{:x}", hash)
    }

    pub struct CommandHelper {
        pub repo_path: PathBuf,
        stdin: Cursor<Vec<u8>>,
        stdout: Cursor<Vec<u8>>,
        stderr: Cursor<Vec<u8>>,
        env: HashMap<String, String>,
    }

    impl CommandHelper {
        pub fn new() -> Self {
            let tmp_dir = TempDir::new().unwrap();
            let repo_path = tmp_dir.path().canonicalize().unwrap();

            CommandHelper {
                repo_path,
                stdin: Cursor::new(vec![]),
                stdout: Cursor::new(vec![]),
                stderr: Cursor::new(vec![]),
                env: HashMap::new(),
            }
        }

        pub fn write_file(&self, name: &str, contents: &str) -> Result<()> {
            let path = self.repo_path.join(name);
            fs::create_dir_all(path.parent().unwrap())?;

            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(&path)?;
            file.write_all(contents.as_bytes())?;

            Ok(())
        }

        pub fn jit_cmd(&mut self, argv: VecDeque<String>) -> Result<()> {
            execute(
                self.repo_path.clone(),
                self.env.clone(),
                argv,
                &mut self.stdin,
                &mut self.stdout,
                &mut self.stderr,
            )
        }

        pub fn assert_index(&self, expected: Vec<(u32, &str)>) -> Result<()> {
            let mut repo = self.repo();
            repo.index.load()?;

            let actual: Vec<(u32, &str)> = repo
                .index
                .entries
                .values()
                .map(|entry| (entry.mode, entry.path.as_str()))
                .collect();

            assert_eq!(actual, expected);

            Ok(())
        }

        fn repo(&self) -> Repository {
            Repository::new(self.repo_path.join(".git"))
        }
    }
}
