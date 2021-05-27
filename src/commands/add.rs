use crate::database::blob::Blob;
use crate::database::object::Object;
use crate::errors::{Error, Result};
use crate::repository::Repository;
use std::env;
use std::io;
use std::path::PathBuf;
use std::process;

pub struct Add;

impl Add {
    pub fn run() -> Result<()> {
        let args: Vec<String> = env::args().collect();

        let root_path = env::current_dir()?;
        let mut repo = Repository::new(root_path.join(".git"));

        if args.len() < 2 {
            eprintln!("Nothing specified, nothing added.");
            process::exit(0);
        }

        match repo.index.load_for_update() {
            Ok(()) => (),
            Err(err) => match err {
                Error::LockDenied(..) => {
                    eprintln!("fatal: {}", err);
                    eprintln!(
                        "
Another jit process seems to be running in this repository.
Please make sure all processes are terminated then try again.
If it still fails, a jit process may have crashed in this
repository earlier: remove the file manually to continue."
                    );
                    process::exit(128);
                }
                _ => return Err(err),
            },
        }

        for path in args[2..].iter() {
            let path = match PathBuf::from(path).canonicalize() {
                Ok(path) => path,
                Err(err) => {
                    if err.kind() == io::ErrorKind::NotFound {
                        eprintln!("fatal: pathspec '{}' did not match any files", path);
                        repo.index.release_lock()?;
                        process::exit(128);
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
                            eprintln!("error: {}", err);
                            eprintln!("fatal: adding files failed");
                            repo.index.release_lock()?;
                            process::exit(128);
                        }
                        _ => return Err(err),
                    },
                };
                let stat = match repo.workspace.stat_file(&path) {
                    Ok(stat) => stat,
                    Err(err) => match err {
                        Error::NoPermission { .. } => {
                            eprintln!("error: {}", err);
                            eprintln!("fatal: adding files failed");
                            repo.index.release_lock()?;
                            process::exit(128);
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
