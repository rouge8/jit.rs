use std::fs;
use std::path::PathBuf;

// TODO: Remove `target` once we have .gitignore support
// TODO: Remove `src` once we have directory support
const IGNORE: &[&str] = &[".", "..", ".git", "target", "src"];

#[derive(Debug)]
pub struct Workspace {
    pathname: PathBuf,
}

impl Workspace {
    pub fn new(pathname: PathBuf) -> Self {
        Workspace { pathname }
    }

    pub fn list_files(&self) -> Vec<PathBuf> {
        let files: Vec<_> = fs::read_dir(&self.pathname)
            .unwrap()
            .filter_map(|path| {
                let path = PathBuf::from(path.unwrap().file_name());

                if self.should_ignore(&path) {
                    return None;
                } else {
                    return Some(path);
                }
            })
            .collect();
        files
    }

    pub fn read_file(&self, path: &PathBuf) -> Vec<u8> {
        fs::read(&self.pathname.join(&path)).unwrap()
    }

    fn should_ignore(&self, path: &PathBuf) -> bool {
        IGNORE
            .iter()
            .any(|ignore_path| path == &PathBuf::from(ignore_path))
    }
}
