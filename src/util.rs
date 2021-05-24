#[cfg(test)]
use rand::distributions::Alphanumeric;
#[cfg(test)]
use rand::{thread_rng, Rng};
#[cfg(test)]
use sha1::{Digest, Sha1};
use std::path::PathBuf;

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

#[cfg(test)]
pub fn random_oid() -> String {
    let rand_string: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect();
    let hash = Sha1::new().chain(rand_string).finalize();

    format!("{:x}", hash)
}
