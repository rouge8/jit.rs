use crate::database::author::Author;
use crate::database::commit::Commit as DatabaseCommit;
use crate::database::entry::Entry;
use crate::database::object::Object;
use crate::database::tree::Tree;
use crate::errors::Result;
use crate::repository::Repository;
use chrono::Local;
use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process;

pub struct Commit;

impl Commit {
    pub fn run<I: Read, O: Write, E: Write>(
        dir: PathBuf,
        env: HashMap<String, String>,
        _argv: VecDeque<String>,
        mut stdin: I,
        mut stdout: O,
        mut stderr: E,
    ) -> Result<()> {
        let mut repo = Repository::new(dir.join(".git"));

        repo.index.load()?;

        let entries = repo.index.entries.values().map(Entry::from).collect();
        let root = Tree::build(entries);
        root.traverse(&|tree| {
            repo.database.store(tree).unwrap();
        });

        let parent = repo.refs.read_head()?;
        let name = &env["GIT_AUTHOR_NAME"];
        let email = &env["GIT_AUTHOR_EMAIL"];
        let author = Author::new(name.clone(), email.clone(), Local::now());
        let mut message = String::new();
        stdin.read_to_string(&mut message)?;

        message = message.trim().to_string();
        if message.is_empty() {
            writeln!(stderr, "Aborting commit due to empty commit message.")?;
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
        writeln!(
            stdout,
            "[{}{}] {}",
            is_root,
            commit.oid(),
            commit.message.lines().next().unwrap(),
        )?;

        Ok(())
    }
}
