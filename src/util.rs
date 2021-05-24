#[cfg(test)]
use rand::distributions::Alphanumeric;
#[cfg(test)]
use rand::{thread_rng, Rng};
#[cfg(test)]
use sha1::{Digest, Sha1};

pub fn is_executable(mode: u32) -> bool {
    mode & 0o1111 != 0
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
