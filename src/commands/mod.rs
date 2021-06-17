use crate::errors::{Error, Result};
use crate::repository::Repository;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::path::PathBuf;

mod add;
mod commit;
mod diff;
mod init;
mod status;

use add::Add;
use commit::Commit;
use diff::Diff;
use init::Init;
use status::Status;

pub fn execute<O: Write, E: Write>(
    dir: PathBuf,
    env: HashMap<String, String>,
    mut argv: VecDeque<String>,
    stdout: O,
    stderr: E,
) -> Result<()> {
    let name = if let Some(name) = argv.pop_front() {
        name
    } else {
        String::from("")
    };

    let ctx = CommandContext::new(dir, env, argv, stdout, stderr);

    match name.as_str() {
        "init" => {
            let cmd = Init::new(ctx);
            cmd.run()
        }
        "add" => {
            let mut cmd = Add::new(ctx);
            cmd.run()
        }
        "commit" => {
            let mut cmd = Commit::new(ctx);
            cmd.run()
        }
        "status" => {
            let mut cmd = Status::new(ctx);
            cmd.run()
        }
        "diff" => {
            let mut cmd = Diff::new(ctx);
            cmd.run()
        }
        _ => Err(Error::UnknownCommand(name.to_string())),
    }
}

pub struct CommandContext<O: Write, E: Write> {
    dir: PathBuf,
    env: HashMap<String, String>,
    argv: VecDeque<String>,
    repo: Repository,
    stdout: RefCell<O>,
    stderr: RefCell<E>,
}

impl<O: Write, E: Write> CommandContext<O, E> {
    pub fn new(
        dir: PathBuf,
        env: HashMap<String, String>,
        argv: VecDeque<String>,
        stdout: O,
        stderr: E,
    ) -> Self {
        let repo = Repository::new(dir.join(".git"));

        Self {
            dir,
            env,
            argv,
            repo,
            stdout: (RefCell::new(stdout)),
            stderr: (RefCell::new(stderr)),
        }
    }
}
