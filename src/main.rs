use chrono::Local;
use std::env;
use std::fs;
use std::io;
use std::io::Read;
use std::process;

mod database;
mod lockfile;
mod refs;
mod workspace;
use database::author::Author;
use database::blob::Blob;
use database::commit::Commit;
use database::entry::Entry;
use database::object::Object;
use database::tree::Tree;
use database::Database;
use refs::Refs;
use workspace::Workspace;

fn main() {
    let args: Vec<String> = env::args().collect();

    let command = if let Some(command) = args.get(1) {
        command.as_str()
    } else {
        ""
    };

    match command {
        "init" => {
            let cwd = env::current_dir().unwrap();
            let root_path = if let Some(path) = args.get(2) {
                cwd.join(path)
            } else {
                cwd
            };

            let git_path = root_path.join(".git");

            for dir in ["objects", "refs"].iter() {
                fs::create_dir_all(git_path.join(dir)).unwrap_or_else(|err| {
                    eprintln!("fatal: {}", err);
                    process::exit(1);
                });
            }

            println!("Initialized empty Jit repository in {:?}", git_path);
        }
        "commit" => {
            let root_path = env::current_dir().unwrap();
            let git_path = root_path.join(".git");
            let db_path = git_path.join("objects");

            let workspace = Workspace::new(root_path);
            let database = Database::new(db_path);
            let refs = Refs::new(git_path);

            let entries: Vec<Entry> = workspace
                .list_files()
                .iter()
                .map(|path| {
                    let data = workspace.read_file(&path);
                    let blob = Blob::new(data);

                    database.store(&blob);

                    let mode = workspace.file_mode(&path);
                    Entry::new(&path, blob.oid(), mode)
                })
                .collect();

            let root = Tree::build(entries);
            root.traverse(&|tree| {
                database.store(tree);
            });

            let parent = refs.read_head();
            let name = env::var("GIT_AUTHOR_NAME").unwrap();
            let email = env::var("GIT_AUTHOR_EMAIL").unwrap();
            let author = Author::new(name, email, Local::now());
            let mut message = String::new();
            let mut stdin = io::stdin();
            stdin.read_to_string(&mut message).unwrap();

            let commit = Commit::new(parent, root.oid(), author, message);
            database.store(&commit);
            refs.update_head(commit.oid()).unwrap();

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
        _ => {
            eprintln!("jit: '{}' is not a jit command.", command);
            process::exit(1);
        }
    }
}
