use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Output;
use std::{fs, io};

use assert_cmd::prelude::OutputAssertExt;
use assert_cmd::Command;
use filetime::FileTime;
use is_executable::IsExecutable;
use jit::database::commit::Commit;
use jit::errors::{Error, Result};
use jit::repository::Repository;
use jit::revision::Revision;
use jit::util::path_to_string;
use rstest::fixture;
use tempfile::TempDir;

pub struct CommandHelper {
    pub repo_path: PathBuf,
    pub repo: Repository,
    pub env: HashMap<String, String>,
    pub stdin: String,
    stdout: Option<String>,
    pub head_oid: Option<String>,
}

#[fixture]
pub fn helper() -> CommandHelper {
    let mut helper = CommandHelper::new();
    helper.init();

    helper
}

impl CommandHelper {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let tmp_dir = TempDir::new().unwrap();
        let repo_path = tmp_dir.into_path().canonicalize().unwrap();
        let repo = Repository::new(repo_path.join(".git"));

        CommandHelper {
            repo_path,
            repo,
            env: HashMap::new(),
            stdin: String::from(""),
            stdout: None,
            head_oid: None,
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

    pub fn mkdir(&self, name: &str) -> Result<()> {
        fs::create_dir_all(self.repo_path.join(name))?;

        Ok(())
    }

    pub fn make_executable(&self, name: &str) -> Result<()> {
        let path = self.repo_path.join(name);
        let mut perms = fs::metadata(&path)?.permissions();

        perms.set_mode(0o755);
        fs::set_permissions(&path, perms)?;

        Ok(())
    }

    pub fn make_unreadable(&self, name: &str) -> Result<()> {
        let path = self.repo_path.join(name);
        let mut perms = fs::metadata(&path)?.permissions();

        perms.set_mode(0o200);
        fs::set_permissions(path, perms)?;

        Ok(())
    }

    pub fn touch(&self, name: &str) -> Result<()> {
        let path = self.repo_path.join(name);

        filetime::set_file_mtime(path, FileTime::now())?;

        Ok(())
    }

    /// Delete the file or directory at `name`, erroring if `name` does not exist.
    pub fn delete(&self, name: &str) -> Result<()> {
        let path = self.repo_path.join(name);

        if path.is_dir() {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }

        Ok(())
    }

    /// Delete the file or directory at `name`. Succeeds even if `name` does not exist.
    pub fn force_delete(&self, name: &str) -> Result<()> {
        match self.delete(name) {
            Ok(()) => (),
            Err(err) => match err {
                Error::Io(err) => {
                    if err.kind() != io::ErrorKind::NotFound {
                        return Err(Error::Io(err));
                    }
                }
                _ => return Err(err),
            },
        }

        Ok(())
    }

    pub fn jit_cmd(&mut self, argv: &[&str]) -> Output {
        let result = Command::cargo_bin(env!("CARGO_PKG_NAME"))
            .unwrap()
            .args(argv)
            .current_dir(&self.repo_path)
            .envs(&self.env)
            .write_stdin(self.stdin.as_bytes())
            .output()
            .unwrap();

        self.stdout = Some(String::from_utf8(result.stdout.clone()).unwrap());

        result
    }

    pub fn init(&mut self) {
        self.jit_cmd(&["init", path_to_string(&self.repo_path).as_str()])
            .assert()
            .code(0);
    }

    pub fn commit(&mut self, message: &str) {
        self.env
            .insert(String::from("GIT_AUTHOR_NAME"), String::from("A. U. Thor"));
        self.env.insert(
            String::from("GIT_AUTHOR_EMAIL"),
            String::from("author@example.com"),
        );

        self.jit_cmd(&["commit", "-m", message]).assert().code(0);
    }

    pub fn assert_status(&mut self, expected: &'static str) {
        self.jit_cmd(&["status", "--porcelain"])
            .assert()
            .stdout(expected);
    }

    pub fn assert_diff(&mut self, expected: &'static str) {
        self.jit_cmd(&["diff"]).assert().stdout(expected);
    }

    pub fn assert_diff_cached(&mut self, expected: &'static str) {
        self.jit_cmd(&["diff", "--cached"])
            .assert()
            .stdout(expected);
    }

    pub fn assert_index(&mut self, contents: &HashMap<&str, &str>) -> Result<()> {
        let mut files = HashMap::new();

        self.repo.index.load()?;

        for entry in self.repo.index.entries.values() {
            let blob = self.repo.database.load_blob(&entry.oid)?;
            files.insert(
                entry.path.clone(),
                std::str::from_utf8(&blob.data)
                    .expect("Invalid UTF-8")
                    .to_string(),
            );
        }

        let contents: HashMap<_, _> = contents
            .iter()
            .map(|(key, val)| (key.to_string(), val.to_string()))
            .collect();
        assert_eq!(files, contents);

        Ok(())
    }

    pub fn assert_workspace(&self, contents: &HashMap<&str, &str>) -> Result<()> {
        let mut expected = HashMap::new();
        for (name, data) in contents {
            expected.insert(name.to_string(), data.to_string());
        }

        let mut files = HashMap::new();

        for pathname in self.repo.workspace.list_files(&self.repo_path)? {
            files.insert(
                path_to_string(&pathname),
                String::from_utf8(self.repo.workspace.read_file(&pathname)?).unwrap(),
            );
        }

        assert_eq!(files, expected);

        Ok(())
    }

    pub fn assert_noent(&self, filename: &str) {
        assert!(!self.repo_path.join(filename).exists());
    }

    pub fn assert_executable(&self, filename: &str) {
        assert!(self.repo_path.join(filename).is_executable());
    }

    pub fn assert_stdout(&self, stdout: &str) {
        assert_eq!(self.stdout.as_ref().expect("no stdout found"), stdout);
    }

    pub fn resolve_revision(&self, expression: &str) -> Result<String> {
        Revision::new(&self.repo, expression).resolve(None)
    }

    pub fn load_commit(&self, expression: &str) -> Result<Commit> {
        Ok(self
            .repo
            .database
            .load_commit(&self.resolve_revision(expression)?)?)
    }
}

impl Drop for CommandHelper {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.repo_path).unwrap();
    }
}
