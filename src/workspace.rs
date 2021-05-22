use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

// TODO: Remove `target` once we have .gitignore support
const IGNORE: &[&str] = &[".", "..", ".git", "target"];

#[derive(Debug)]
pub struct Workspace {
    pathname: PathBuf,
}

impl Workspace {
    pub fn new(pathname: PathBuf) -> Self {
        Workspace { pathname }
    }

    pub fn list_files(&self) -> Vec<PathBuf> {
        self.list_files_at_path(&self.pathname)
    }

    pub fn read_file(&self, path: &PathBuf) -> Vec<u8> {
        fs::read(&self.pathname.join(&path)).unwrap()
    }

    pub fn stat_file(&self, path: &PathBuf) -> fs::Metadata {
        fs::metadata(&self.pathname.join(&path)).unwrap()
    }

    pub fn file_mode(&self, path: &PathBuf) -> u32 {
        self.stat_file(&path).mode()
    }

    fn should_ignore(&self, path: &PathBuf) -> bool {
        IGNORE
            .iter()
            .any(|ignore_path| path == &PathBuf::from(ignore_path))
    }

    fn list_files_at_path(&self, path: &PathBuf) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = Vec::new();

        for entry in fs::read_dir(&path).unwrap() {
            let path = entry.unwrap().path();
            let relative_path = path.strip_prefix(&self.pathname).unwrap().to_path_buf();

            if self.should_ignore(&relative_path) {
                continue;
            }
            if path.is_dir() {
                let mut nested = self.list_files_at_path(&path);
                files.append(&mut nested);
            } else {
                files.push(relative_path);
            }
        }
        files
    }
}
