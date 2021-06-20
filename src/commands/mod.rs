use crate::errors::{Error, Result};
use crate::pager::Pager;
use crate::repository::Repository;
use atty::Stream;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::path::PathBuf;

mod add;
mod branch;
mod commit;
mod diff;
mod init;
mod status;

use add::Add;
use branch::Branch;
use commit::Commit;
use diff::Diff;
use init::Init;
use status::Status;

pub fn execute<O: Write + 'static, E: Write>(
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

    let ctx = CommandContext::new(dir, env, argv, Box::new(stdout), stderr);

    match name.as_str() {
        "add" => {
            let mut cmd = Add::new(ctx);
            cmd.run()
        }
        "branch" => {
            let mut cmd = Branch::new(ctx);
            cmd.run()
        }
        "commit" => {
            let mut cmd = Commit::new(ctx);
            cmd.run()
        }
        "diff" => {
            let mut cmd = Diff::new(ctx);
            cmd.run()
        }
        "init" => {
            let cmd = Init::new(ctx);
            cmd.run()
        }
        "status" => {
            let mut cmd = Status::new(ctx);
            cmd.run()
        }
        _ => Err(Error::UnknownCommand(name.to_string())),
    }
}

pub struct CommandContext<E: Write> {
    dir: PathBuf,
    env: HashMap<String, String>,
    argv: VecDeque<String>,
    repo: Repository,
    stdout: RefCell<Box<dyn Write>>,
    stderr: RefCell<E>,
    using_pager: bool,
}

impl<E: Write> CommandContext<E> {
    pub fn new(
        dir: PathBuf,
        env: HashMap<String, String>,
        argv: VecDeque<String>,
        stdout: Box<dyn Write>,
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
            using_pager: false,
        }
    }

    pub fn setup_pager(&mut self) {
        // Only setup the pager once
        if self.using_pager {
            return;
        }

        // Only setup the pager if stdout is a tty
        if !atty::is(Stream::Stdout) {
            return;
        }

        self.stdout = RefCell::new(Box::new(Pager::new(&self.env)));
        self.using_pager = true;
    }
}
