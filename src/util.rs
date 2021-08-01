use std::path::{Path, PathBuf};

pub fn is_executable(mode: u32) -> bool {
    mode & 0o1111 != 0
}

/// Return the parent directories of `path` in ascending order, e.g.:
///
/// ```
/// # use jit::util::parent_directories;
/// # use std::path::{Path, PathBuf};
/// assert_eq!(
///     parent_directories(Path::new("outer/inner/f.txt")),
///     vec![PathBuf::from("outer/inner"), PathBuf::from("outer")]
/// );
/// ```
pub fn parent_directories(path: &Path) -> Vec<PathBuf> {
    path.ancestors()
        .filter_map(|p| {
            if p != path && p != Path::new("") {
                Some(p.to_path_buf())
            } else {
                None
            }
        })
        .collect()
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

/// Transpose the rows and columns of `ary`.
///
/// Panics if the length of the subvectors don't match.
///
/// Based on Ruby's `Array.transpose()`
pub fn transpose<T: Clone>(ary: Vec<Vec<T>>) -> Vec<Vec<T>> {
    let alen = ary.len();
    if alen == 0 {
        return Vec::new();
    }

    let elen = ary[0].len();
    let mut result: Vec<_> = (0..elen).map(|_| Vec::with_capacity(alen)).collect();

    for tmp in &ary {
        if tmp.len() != elen {
            panic!("element size differs ({} should be {})", tmp.len(), elen);
        }
        for j in 0..elen {
            result[j].push(tmp[j].clone());
        }
    }

    result
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};
    use rstest::rstest;
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

    #[test]
    fn transpose_works() {
        let ary = vec![
            vec![1, 2, 3],
            vec![4, 5, 6],
            vec![7, 8, 9],
            vec![10, 11, 12],
        ];
        let expected = vec![vec![1, 4, 7, 10], vec![2, 5, 8, 11], vec![3, 6, 9, 12]];

        assert_eq!(transpose(ary), expected);
    }

    #[rstest]
    #[case("outer/inner/f.txt", &["outer/inner", "outer"])]
    #[case("/outer/inner/f.txt", &["/outer/inner", "/outer", "/"])]
    #[case("f.txt", &[])]
    fn parent_directories_works(#[case] input: &str, #[case] expected: &[&str]) {
        let expected: Vec<_> = expected.iter().map(PathBuf::from).collect();

        assert_eq!(parent_directories(Path::new(input)), expected);
    }
}
