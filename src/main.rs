use anyhow::Result;
use chrono::Local;
use std::env;
use std::fs;
use std::io;
use std::io::Read;
use std::path::PathBuf;
use std::process;

mod database;
mod errors;
mod index;
mod lockfile;
mod refs;
mod repository;
mod util;
mod workspace;
use database::author::Author;
use database::blob::Blob;
use database::commit::Commit;
use database::entry::Entry;
use database::object::Object;
use database::tree::Tree;
use errors::Error;
use repository::Repository;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    let command = if let Some(command) = args.get(1) {
        command.as_str()
    } else {
        ""
    };

    match command {
        "init" => {
            let cwd = env::current_dir()?;
            let root_path = if let Some(path) = args.get(2) {
                cwd.join(path)
            } else {
                cwd
            };

            let git_path = root_path.join(".git");

            for dir in ["objects", "refs"].iter() {
                fs::create_dir_all(git_path.join(dir))?;
            }

            println!("Initialized empty Jit repository in {:?}", git_path);
        }
        "commit" => {
            let root_path = env::current_dir()?;
            let mut repo = Repository::new(root_path.join(".git"));

            repo.index.load()?;

            let entries = repo.index.entries.values().map(Entry::from).collect();
            let root = Tree::build(entries);
            root.traverse(&|tree| {
                repo.database.store(tree).unwrap();
            });

            let parent = repo.refs.read_head()?;
            let name = env::var("GIT_AUTHOR_NAME")?;
            let email = env::var("GIT_AUTHOR_EMAIL")?;
            let author = Author::new(name, email, Local::now());
            let mut message = String::new();
            let mut stdin = io::stdin();
            stdin.read_to_string(&mut message)?;

            message = message.trim().to_string();
            if message.is_empty() {
                eprintln!("Aborting commit due to empty commit message.");
                process::exit(0);
            }

            let commit = Commit::new(parent, root.oid(), author, message);
            repo.database.store(&commit)?;
            repo.refs.update_head(commit.oid())?;

            let mut is_root = String::new();
            match commit.parent {
                Some(_) => (),
                None => is_root.push_str("(root-commit) "),
            }
            println!(
                "[{}{}] {}",
                is_root,
                commit.oid(),
                commit.message.lines().next().unwrap(),
            );
        }
        "add" => {
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
                    _ => return Err(anyhow::Error::from(err)),
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
                            return Err(anyhow::Error::from(err));
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
                            _ => return Err(anyhow::Error::from(err)),
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
                            _ => return Err(anyhow::Error::from(err)),
                        },
                    };

                    let blob = Blob::new(data);
                    repo.database.store(&blob)?;
                    repo.index.add(path, blob.oid(), stat);
                }
            }

            repo.index.write_updates()?;
        }
        _ => {
            eprintln!("jit: '{}' is not a jit command.", command);
            process::exit(1);
        }
    }

    Ok(())
}
