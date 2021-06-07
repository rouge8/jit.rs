use assert_cmd::prelude::OutputAssertExt;
use assert_cmd::Command;
use filetime::FileTime;
use jit::errors::Result;
use jit::repository::Repository;
use jit::util::path_to_string;
use rstest::fixture;
use std::collections::HashMap;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Output;
use tempfile::TempDir;

pub struct CommandHelper {
    pub repo_path: PathBuf,
    env: HashMap<&'static str, &'static str>,
    stdin: &'static str,
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

        CommandHelper {
            repo_path,
            env: HashMap::new(),
            stdin: "",
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
        fs::set_permissions(path, perms)?;

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

    pub fn delete(&self, name: &str) -> Result<()> {
        let path = self.repo_path.join(name);

        if path.is_dir() {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }

        Ok(())
    }

    pub fn jit_cmd(&mut self, argv: &[&str]) -> Output {
        Command::cargo_bin(env!("CARGO_PKG_NAME"))
            .unwrap()
            .args(argv)
            .current_dir(&self.repo_path)
            .envs(&self.env)
            .write_stdin(self.stdin.as_bytes())
            .output()
            .unwrap()
    }

    pub fn init(&mut self) {
        self.jit_cmd(&["init", path_to_string(&self.repo_path).as_str()])
            .assert()
            .code(0);
    }

    pub fn commit(&mut self, message: &'static str) {
        self.env.insert("GIT_AUTHOR_NAME", "A. U. Thor");
        self.env.insert("GIT_AUTHOR_EMAIL", "author@example.com");
        self.stdin = message;

        self.jit_cmd(&["commit"]);
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

    pub fn assert_status(&mut self, expected: &'static str) {
        self.jit_cmd(&["status", "--porcelain"])
            .assert()
            .stdout(expected);
    }

    pub fn repo(&self) -> Repository {
        Repository::new(self.repo_path.join(".git"))
    }
}

impl Drop for CommandHelper {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.repo_path).unwrap();
    }
}
