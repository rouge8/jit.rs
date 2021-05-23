use anyhow::Result;
use chrono::Local;
use std::env;
use std::fs;
use std::io;
use std::io::Read;
use std::path::PathBuf;
use std::process;

mod database;
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
            let db_path = git_path.join("objects");

            let workspace = Workspace::new(root_path);
            let database = Database::new(db_path);
            let refs = Refs::new(git_path);

            let entries = workspace
                .list_files()?
                .iter()
                .map(|path| {
                    let data = workspace.read_file(&path)?;
                    let blob = Blob::new(data);

                    database.store(&blob)?;

                    let mode = workspace.file_mode(&path)?;
                    Ok(Entry::new(&path, blob.oid(), mode))
                })
                .collect::<Result<Vec<Entry>>>()?;

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

            index.load_for_update()?;

            for path in args[2..].iter() {
                let path = PathBuf::from(path).canonicalize()?;
                for path in workspace.list_files_at_path(&path)? {
                    let data = workspace.read_file(&path)?;
                    let stat = workspace.stat_file(&path)?;

                    let blob = Blob::new(data);
                    database.store(&blob)?;
                    index.add(path, blob.oid(), stat);
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
