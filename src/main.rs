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
mod util;
mod workspace;
use database::author::Author;
use database::blob::Blob;
use database::commit::Commit;
use database::entry::Entry;
use database::object::Object;
use database::tree::Tree;
use database::Database;
use errors::Error;
use index::Index;
use refs::Refs;
use workspace::Workspace;

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
            let git_path = root_path.join(".git");

            let database = Database::new(git_path.join("objects"));
            let mut index = Index::new(git_path.join("index"));
            let refs = Refs::new(git_path);

            index.load()?;

            let entries = index.entries.values().map(Entry::from).collect();
            let root = Tree::build(entries);
            root.traverse(&|tree| {
                database.store(tree).unwrap();
            });

            let parent = refs.read_head()?;
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
            database.store(&commit)?;
            refs.update_head(commit.oid())?;

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
            let git_path = root_path.join(".git");

            let workspace = Workspace::new(root_path);
            let database = Database::new(git_path.join("objects"));
            let mut index = Index::new(git_path.join("index"));

            if args.len() < 2 {
                eprintln!("Nothing specified, nothing added.");
                process::exit(0);
            }

            match index.load_for_update() {
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
                match PathBuf::from(path).canonicalize() {
                    Ok(path) => {
                        for path in workspace.list_files(&path)? {
                            let data = match workspace.read_file(&path) {
                                Ok(data) => data,
                                Err(err) => match err {
                                    Error::NoPermission { .. } => {
                                        eprintln!("error: {}", err);
                                        eprintln!("fatal: adding files failed");
                                        index.release_lock()?;
                                        process::exit(128);
                                    }
                                    _ => return Err(anyhow::Error::from(err)),
                                },
                            };
                            let stat = match workspace.stat_file(&path) {
                                Ok(stat) => stat,
                                Err(err) => match err {
                                    Error::NoPermission { .. } => {
                                        eprintln!("error: {}", err);
                                        eprintln!("fatal: adding files failed");
                                        index.release_lock()?;
                                        process::exit(128);
                                    }
                                    _ => return Err(anyhow::Error::from(err)),
                                },
                            };

                            let blob = Blob::new(data);
                            database.store(&blob)?;
                            index.add(path, blob.oid(), stat);
                        }
                    }
                    Err(err) => {
                        if err.kind() == io::ErrorKind::NotFound {
                            eprintln!("fatal: pathspec '{}' did not match any files", path);
                            index.release_lock()?;
                            process::exit(128);
                        } else {
                            return Err(anyhow::Error::from(err));
                        }
                    }
                }
            }

            index.write_updates()?;
        }
        _ => {
            eprintln!("jit: '{}' is not a jit command.", command);
            process::exit(1);
        }
    }

    Ok(())
}
