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
