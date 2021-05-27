use crate::database::author::Author;
use crate::database::commit::Commit as DatabaseCommit;
use crate::database::entry::Entry;
use crate::database::object::Object;
use crate::database::tree::Tree;
use crate::errors::Result;
use crate::repository::Repository;
use chrono::Local;
use std::env;
use std::io;
use std::io::Read;
use std::process;

pub struct Commit;

impl Commit {
    pub fn run() -> Result<()> {
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

        let commit = DatabaseCommit::new(parent, root.oid(), author, message);
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

        Ok(())
    }
}
