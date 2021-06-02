use crate::commands::CommandContext;
use crate::database::blob::Blob;
use crate::database::object::Object;
use crate::errors::{Error, Result};
use crate::repository::Repository;
use std::io;
use std::io::Read;
use std::path::PathBuf;

pub struct Add;

impl Add {
    pub fn run<I: Read>(mut ctx: CommandContext<I>) -> Result<()> {
        if ctx.argv.is_empty() {
            eprintln!("Nothing specified, nothing added.");
            return Err(Error::Exit(0));
        }

        match ctx.repo.index.load_for_update() {
            Ok(()) => (),
            Err(err) => return Self::handle_locked_index(err),
        }

        for path in ctx.argv.range(0..) {
            let path = match PathBuf::from(path).canonicalize() {
                Ok(path) => path,
                Err(err) => return Self::handle_missing_file(ctx.repo, path, err),
            };

            for path in ctx.repo.workspace.list_files(&path)? {
                Self::add_to_index(&mut ctx.repo, path)?;
            }
        }

        ctx.repo.index.write_updates()?;

        Ok(())
    }

    fn add_to_index(repo: &mut Repository, path: PathBuf) -> Result<()> {
        let data = match repo.workspace.read_file(&path) {
            Ok(data) => data,
            Err(err) => return Self::handle_unreadable_file(repo, err),
        };
        let stat = match repo.workspace.stat_file(&path) {
            Ok(stat) => stat,
            Err(err) => return Self::handle_unreadable_file(repo, err),
        };

        let blob = Blob::new(data);
        repo.database.store(&blob)?;
        repo.index.add(path, blob.oid(), stat);

        Ok(())
    }

    fn handle_locked_index(err: Error) -> Result<()> {
        match err {
            Error::LockDenied(..) => {
                eprintln!("fatal: {}", err);
                eprintln!(
                    "
Another jit process seems to be running in this repository.
Please make sure all processes are terminated then try again.
If it still fails, a jit process may have crashed in this
repository earlier: remove the file manually to continue."
                );
                Err(Error::Exit(128))
            }
            _ => Err(err),
        }
    }

    fn handle_missing_file(mut repo: Repository, path: &str, err: io::Error) -> Result<()> {
        if err.kind() == io::ErrorKind::NotFound {
            eprintln!("fatal: pathspec '{}' did not match any files", path);
            repo.index.release_lock()?;
            Err(Error::Exit(128))
        } else {
            Err(Error::Io(err))
        }
    }

    fn handle_unreadable_file(repo: &mut Repository, err: Error) -> Result<()> {
        match err {
            Error::NoPermission { .. } => {
                eprintln!("error: {}", err);
                eprintln!("fatal: adding files failed");
                repo.index.release_lock()?;
                Err(Error::Exit(128))
            }
            _ => Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::errors::Result;
    use crate::util::tests::CommandHelper;
    use assert_cmd::prelude::OutputAssertExt;

    #[test]
    fn add_a_regular_file_to_the_index() -> Result<()> {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.write_file("hello.txt", "hello")?;

        helper.jit_cmd(&["add", "hello.txt"]);

        helper.assert_index(vec![(0o100644, "hello.txt")]).unwrap();

        Ok(())
    }

    #[test]
    fn add_an_executable_file_to_the_index() -> Result<()> {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.write_file("hello.txt", "hello")?;
        helper.make_executable("hello.txt")?;

        helper.jit_cmd(&["add", "hello.txt"]);

        helper.assert_index(vec![(0o100755, "hello.txt")]).unwrap();

        Ok(())
    }

    #[test]
    fn add_multiple_files_to_the_index() -> Result<()> {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.write_file("hello.txt", "hello")?;
        helper.write_file("world.txt", "world")?;

        helper.jit_cmd(&["add", "hello.txt", "world.txt"]);

        helper
            .assert_index(vec![(0o100644, "hello.txt"), (0o100644, "world.txt")])
            .unwrap();

        Ok(())
    }

    #[test]
    fn incrementally_add_files_to_the_index() -> Result<()> {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.write_file("hello.txt", "hello")?;
        helper.write_file("world.txt", "world")?;

        helper.jit_cmd(&["add", "hello.txt"]);

        helper.assert_index(vec![(0o100644, "hello.txt")]).unwrap();

        helper.jit_cmd(&["add", "world.txt"]);

        helper
            .assert_index(vec![(0o100644, "hello.txt"), (0o100644, "world.txt")])
            .unwrap();

        Ok(())
    }

    #[test]
    fn add_a_directory_to_the_index() -> Result<()> {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.write_file("a-dir/nested.txt", "content")?;

        helper.jit_cmd(&["add", "a-dir"]);

        helper
            .assert_index(vec![(0o100644, "a-dir/nested.txt")])
            .unwrap();

        Ok(())
    }

    #[test]
    fn add_the_repository_root_to_the_index() -> Result<()> {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.write_file("a/b/c/file.txt", "content")?;

        helper.jit_cmd(&["add", "."]);

        helper
            .assert_index(vec![(0o100644, "a/b/c/file.txt")])
            .unwrap();

        Ok(())
    }

    #[test]
    fn silent_on_success() -> Result<()> {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.write_file("hello.txt", "hello")?;

        helper
            .jit_cmd(&["add", "hello.txt"])
            .assert()
            .code(0)
            .stdout("")
            .stderr("");

        Ok(())
    }

    #[test]
    fn fail_for_non_existent_files() -> Result<()> {
        let mut helper = CommandHelper::new();
        helper.init();

        helper
            .jit_cmd(&["add", "no-such-file"])
            .assert()
            .code(128)
            .stdout("")
            .stderr("fatal: pathspec 'no-such-file' did not match any files\n");
        helper.assert_index(vec![]).unwrap();

        Ok(())
    }

    #[test]
    fn fail_for_unreadable_files() -> Result<()> {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.write_file("secret.txt", "")?;
        helper.make_unreadable("secret.txt")?;

        helper
            .jit_cmd(&["add", "secret.txt"])
            .assert()
            .code(128)
            .stdout("")
            .stderr("error: open('secret.txt'): Permission denied\nfatal: adding files failed\n");
        helper.assert_index(vec![]).unwrap();

        Ok(())
    }

    #[test]
    fn fail_if_the_index_is_locked() -> Result<()> {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.write_file("file.txt", "")?;
        helper.write_file(".git/index.lock", "")?;

        helper
            .jit_cmd(&["add", "file.txt"])
            .assert()
            .code(128)
            .stdout("");
        helper.assert_index(vec![]).unwrap();

        Ok(())
    }
}
