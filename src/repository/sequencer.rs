use crate::database::commit::Commit;
use crate::database::object::Object;
use crate::database::Database;
use crate::errors::{Error, Result};
use crate::lockfile::Lockfile;
use crate::refs::ORIG_HEAD;
use crate::repository::Repository;
use lazy_static::lazy_static;
use regex::Regex;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;

lazy_static! {
    static ref LOAD_LINE: Regex = Regex::new(r"^(\S+) (\S+) (.*)$").unwrap();
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Pick,
    Revert,
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let action = match self {
            Action::Pick => "pick",
            Action::Revert => "revert",
        };
        write!(f, "{}", action)
    }
}

impl FromStr for Action {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "pick" => Ok(Action::Pick),
            "revert" => Ok(Action::Revert),
            _ => unimplemented!(),
        }
    }
}

#[derive(Debug)]
pub struct Sequencer {
    pub repo: Repository,
    pathname: PathBuf,
    abort_path: PathBuf,
    head_path: PathBuf,
    todo_path: PathBuf,
    todo_file: Option<Lockfile>,
    commands: Vec<(Action, Commit)>,
}

impl Sequencer {
    pub fn new(repo: &Repository) -> Self {
        let pathname = repo.git_path.join("sequencer");
        let abort_path = pathname.join("abort-safety");
        let head_path = pathname.join("head");
        let todo_path = pathname.join("todo");

        Self {
            repo: Repository::new(repo.git_path.clone()),
            pathname,
            abort_path,
            head_path,
            todo_path,
            todo_file: None,
            commands: Vec::new(),
        }
    }

    pub fn start(&mut self) -> Result<()> {
        fs::create_dir(&self.pathname)?;

        let head_oid = self.repo.refs.read_head()?.unwrap();
        self.write_file(&self.head_path, &head_oid)?;
        self.write_file(&self.abort_path, &head_oid)?;

        self.open_todo_file()?;

        Ok(())
    }

    pub fn pick(&mut self, commit: &Commit) {
        self.commands.push((Action::Pick, commit.to_owned()));
    }

    pub fn revert(&mut self, commit: &Commit) {
        self.commands.push((Action::Revert, commit.to_owned()));
    }

    pub fn next_command(&self) -> Option<(Action, Commit)> {
        self.commands
            .first()
            .map(|(action, commit)| (action.to_owned(), commit.to_owned()))
    }

    pub fn drop_command(&mut self) -> Result<()> {
        self.commands.remove(0);
        self.write_file(&self.abort_path, &self.repo.refs.read_head()?.unwrap())?;

        Ok(())
    }

    pub fn load(&mut self) -> Result<()> {
        self.open_todo_file()?;

        if !self.todo_path.is_file() {
            return Ok(());
        }

        for line in fs::read_to_string(&self.todo_path)?.lines() {
            let captures = &LOAD_LINE.captures(line).unwrap();
            let action = &captures[1];
            let oid = &captures[2];
            let oids = self.repo.database.prefix_match(oid)?;
            self.commands.push((
                Action::from_str(action)?,
                self.repo.database.load_commit(&oids[0])?,
            ));
        }

        Ok(())
    }

    pub fn dump(&mut self) -> Result<()> {
        if let Some(todo_file) = &mut self.todo_file {
            for (action, commit) in &self.commands {
                let short = Database::short_oid(&commit.oid());
                writeln!(todo_file, "{} {} {}", action, short, commit.title_line())?;
            }

            todo_file.commit()?;
        }

        Ok(())
    }

    pub fn abort(&mut self) -> Result<()> {
        let head_oid = fs::read_to_string(&self.head_path)?.trim().to_owned();
        let expected = fs::read_to_string(&self.abort_path)?.trim().to_owned();
        let actual = self.repo.refs.read_head()?.unwrap();

        self.quit()?;

        if actual != expected {
            return Err(Error::UnsafeRewind);
        }

        self.repo.hard_reset(&head_oid)?;
        let orig_head = self.repo.refs.update_head(&head_oid)?.unwrap();
        self.repo.refs.update_ref(ORIG_HEAD, &orig_head)?;

        Ok(())
    }

    pub fn quit(&self) -> Result<()> {
        fs::remove_dir_all(&self.pathname)?;

        Ok(())
    }

    fn write_file(&self, path: &Path, content: &str) -> Result<()> {
        let mut lockfile = Lockfile::new(path.to_owned());
        lockfile.hold_for_update()?;
        writeln!(lockfile, "{}", content)?;
        lockfile.commit()?;

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
