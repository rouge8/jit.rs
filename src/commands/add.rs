use crate::database::blob::Blob;
use crate::database::object::Object;
use crate::errors::{Error, Result};
use crate::repository::Repository;
use std::collections::{HashMap, VecDeque};
use std::io;
use std::io::{Read, Write};
use std::path::PathBuf;

pub struct Add;

impl Add {
    pub fn run<I: Read, O: Write, E: Write>(
        dir: PathBuf,
        _env: HashMap<String, String>,
        argv: VecDeque<String>,
        _stdin: I,
        _stdout: O,
        mut stderr: E,
    ) -> Result<()> {
        let mut repo = Repository::new(dir.join(".git"));

        if argv.is_empty() {
            writeln!(stderr, "Nothing specified, nothing added.")?;
            return Err(Error::Exit(0));
        }

        match repo.index.load_for_update() {
            Ok(()) => (),
            Err(err) => match err {
                Error::LockDenied(..) => {
                    writeln!(stderr, "fatal: {}", err)?;
                    writeln!(
                        stderr,
                        "
Another jit process seems to be running in this repository.
Please make sure all processes are terminated then try again.
If it still fails, a jit process may have crashed in this
repository earlier: remove the file manually to continue."
                    )?;
                    return Err(Error::Exit(128));
                }
                _ => return Err(err),
            },
        }

        for path in argv.range(0..) {
            let path = match PathBuf::from(path).canonicalize() {
                Ok(path) => path,
                Err(err) => {
                    if err.kind() == io::ErrorKind::NotFound {
                        writeln!(stderr, "fatal: pathspec '{}' did not match any files", path)?;
                        repo.index.release_lock()?;
                        return Err(Error::Exit(128));
                    } else {
                        return Err(Error::Io(err));
                    }
                }
            };

            for path in repo.workspace.list_files(&path)? {
                let data = match repo.workspace.read_file(&path) {
                    Ok(data) => data,
                    Err(err) => match err {
                        Error::NoPermission { .. } => {
                            writeln!(stderr, "error: {}", err)?;
                            writeln!(stderr, "fatal: adding files failed")?;
                            repo.index.release_lock()?;
                            return Err(Error::Exit(128));
                        }
                        _ => return Err(err),
                    },
                };
                let stat = match repo.workspace.stat_file(&path) {
                    Ok(stat) => stat,
                    Err(err) => match err {
                        Error::NoPermission { .. } => {
                            writeln!(stderr, "error: {}", err)?;
                            writeln!(stderr, "fatal: adding files failed")?;
                            repo.index.release_lock()?;
                            return Err(Error::Exit(128));
                        }
                        _ => return Err(err),
                    },
                };

                let blob = Blob::new(data);
                repo.database.store(&blob)?;
                repo.index.add(path, blob.oid(), stat);
            }
        }

        repo.index.write_updates()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::errors::Result;
    use crate::util::tests::CommandHelper;

    #[test]
    fn add_a_regular_file_to_the_index() -> Result<()> {
        let mut helper = CommandHelper::new();
        helper.init()?;

        helper.write_file("hello.txt", "hello")?;

        helper.jit_cmd(&["add", "hello.txt"])?;

        helper.assert_index(vec![(0o100644, "hello.txt")]).unwrap();

        Ok(())
    }

    #[test]
    fn add_an_executable_file_to_the_index() -> Result<()> {
        let mut helper = CommandHelper::new();
        helper.init()?;

        helper.write_file("hello.txt", "hello")?;
        helper.make_executable("hello.txt")?;

        helper.jit_cmd(&["add", "hello.txt"])?;

        helper.assert_index(vec![(0o100755, "hello.txt")]).unwrap();

        Ok(())
    }

    #[test]
    fn add_multiple_files_to_the_index() -> Result<()> {
        let mut helper = CommandHelper::new();
        helper.init()?;

        helper.write_file("hello.txt", "hello")?;
        helper.write_file("world.txt", "world")?;

        helper.jit_cmd(&["add", "hello.txt", "world.txt"])?;

        helper
            .assert_index(vec![(0o100644, "hello.txt"), (0o100644, "world.txt")])
            .unwrap();

        Ok(())
    }

    #[test]
    fn incrementally_add_files_to_the_index() -> Result<()> {
        let mut helper = CommandHelper::new();
        helper.init()?;

        helper.write_file("hello.txt", "hello")?;
        helper.write_file("world.txt", "world")?;

        helper.jit_cmd(&["add", "hello.txt"])?;

        helper.assert_index(vec![(0o100644, "hello.txt")]).unwrap();

        helper.jit_cmd(&["add", "world.txt"])?;

        helper
            .assert_index(vec![(0o100644, "hello.txt"), (0o100644, "world.txt")])
            .unwrap();

        Ok(())
    }

    #[test]
    fn add_a_directory_to_the_index() -> Result<()> {
        let mut helper = CommandHelper::new();
        helper.init()?;

        helper.write_file("a-dir/nested.txt", "content")?;

        helper.jit_cmd(&["add", "a-dir"])?;

        helper
            .assert_index(vec![(0o100644, "a-dir/nested.txt")])
            .unwrap();

        Ok(())
    }

    #[test]
    fn add_the_repository_root_to_the_index() -> Result<()> {
        let mut helper = CommandHelper::new();
        helper.init()?;

        helper.write_file("a/b/c/file.txt", "content")?;

        helper.jit_cmd(&["add", "."])?;

        helper
            .assert_index(vec![(0o100644, "a/b/c/file.txt")])
            .unwrap();

        Ok(())
    }
}
