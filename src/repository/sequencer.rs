use crate::database::commit::Commit;
use crate::database::object::Object;
use crate::database::Database;
use crate::errors::Result;
use crate::lockfile::Lockfile;
use crate::repository::Repository;
use lazy_static::lazy_static;
use regex::Regex;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

lazy_static! {
    static ref LOAD_LINE: Regex = Regex::new(r"^pick (\S+) (.*)$").unwrap();
}

#[derive(Debug)]
pub struct Sequencer {
    repo: Repository,
    pathname: PathBuf,
    todo_path: PathBuf,
    todo_file: Option<Lockfile>,
    commands: Vec<Commit>,
}

impl Sequencer {
    pub fn new(repo: &Repository) -> Self {
        let pathname = repo.git_path.join("sequencer");
        let todo_path = pathname.join("todo");

        Self {
            repo: Repository::new(repo.git_path.clone()),
            pathname,
            todo_path,
            todo_file: None,
            commands: Vec::new(),
        }
    }

    pub fn start(&mut self) -> Result<()> {
        fs::create_dir(&self.pathname)?;
        self.open_todo_file()?;

        Ok(())
    }

    pub fn pick(&mut self, commit: &Commit) {
        self.commands.push(commit.to_owned());
    }

    pub fn next_command(&self) -> Option<Commit> {
        self.commands.first().map(|commit| commit.to_owned())
    }

    pub fn drop_command(&mut self) {
        self.commands.remove(0);
    }

    pub fn load(&mut self) -> Result<()> {
        self.open_todo_file()?;

        if !self.todo_path.is_file() {
            return Ok(());
        }

        for line in fs::read_to_string(&self.todo_path)?.lines() {
            let oid = &LOAD_LINE.captures(line).unwrap()[1];
            let oids = self.repo.database.prefix_match(oid)?;
            self.commands
                .push(self.repo.database.load_commit(&oids[0])?);
        }

        Ok(())
    }

    pub fn dump(&mut self) -> Result<()> {
        if let Some(todo_file) = &mut self.todo_file {
            for commit in &self.commands {
                let short = Database::short_oid(&commit.oid());
                writeln!(todo_file, "pick {} {}", short, commit.title_line())?;
            }

            todo_file.commit()?;
        }

        Ok(())
    }

    pub fn quit(&self) -> Result<()> {
        fs::remove_dir_all(&self.pathname)?;

        Ok(())
    }

    fn open_todo_file(&mut self) -> Result<()> {
        if !self.pathname.is_dir() {
            return Ok(());
        }

        self.todo_file = Some(Lockfile::new(self.todo_path.clone()));
        self.todo_file.as_mut().unwrap().hold_for_update()?;

        Ok(())
    }
}
