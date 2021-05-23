use anyhow::Result;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

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

    pub fn list_files(&self) -> Result<Vec<PathBuf>> {
        let files = self.list_files_at_path(&self.pathname)?;
        Ok(files)
    }

    pub fn read_file(&self, path: &Path) -> Result<Vec<u8>> {
        Ok(fs::read(&self.pathname.join(&path))?)
    }

    pub fn stat_file(&self, path: &Path) -> Result<fs::Metadata> {
        Ok(fs::metadata(&self.pathname.join(&path))?)
    }

    pub fn file_mode(&self, path: &Path) -> Result<u32> {
        Ok(self.stat_file(&path)?.mode())
    }

    fn should_ignore(&self, path: &Path) -> bool {
        IGNORE
            .iter()
            .any(|ignore_path| path == PathBuf::from(ignore_path))
    }

    pub fn list_files_at_path(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let relative_path = path.strip_prefix(&self.pathname)?;

        if self.should_ignore(&relative_path) {
            Ok(vec![])
        } else if relative_path.is_file() {
            Ok(vec![relative_path.to_path_buf()])
        } else {
            let mut files: Vec<PathBuf> = Vec::new();

            for entry in fs::read_dir(&path)? {
                let path = entry?.path();
                let mut nested = self.list_files_at_path(&path)?;
                files.append(&mut nested);
            }
            Ok(files)
        }
    }
}
