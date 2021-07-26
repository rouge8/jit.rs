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

/// Iterator yielding every line in a string. The line includes newline character(s).
///
/// From <https://stackoverflow.com/a/40457615/609144>
pub struct LinesWithEndings<'a> {
    input: &'a str,
}

impl<'a> LinesWithEndings<'a> {
    pub fn from(input: &'a str) -> LinesWithEndings<'a> {
        LinesWithEndings { input }
    }
}

impl<'a> Iterator for LinesWithEndings<'a> {
    type Item = &'a str;

    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        if self.input.is_empty() {
            return None;
        }
        let split = self
            .input
            .find('\n')
            .map(|i| i + 1)
            .unwrap_or(self.input.len());
        let (line, rest) = self.input.split_at(split);
        self.input = rest;
        Some(line)
    }
}

#[cfg(test)]
pub mod tests {
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};
    use sha1::{Digest, Sha1};

    pub fn random_oid() -> String {
        let rand_string: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();
        let hash = Sha1::new().chain(rand_string).finalize();

        format!("{:x}", hash)
    }
}
