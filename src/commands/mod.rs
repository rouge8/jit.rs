use crate::errors::{Error, Result};
use crate::repository::Repository;
use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::path::PathBuf;

mod add;
mod commit;
mod init;
mod status;

use add::Add;
use commit::Commit;
use init::Init;
use status::Status;

pub fn execute<I: Read>(
    dir: PathBuf,
    env: HashMap<String, String>,
    mut argv: VecDeque<String>,
    stdin: I,
) -> Result<()> {
    let name = if let Some(name) = argv.pop_front() {
        name
    } else {
        String::from("")
    };

    let ctx = CommandContext::new(dir, env, argv, stdin);
    let command = match name.as_str() {
        "init" => Init::run,
        "add" => Add::run,
        "commit" => Commit::run,
        "status" => Status::run,
        _ => return Err(Error::UnknownCommand(name.to_string())),
    };

    command(ctx)
}

pub struct CommandContext<I>
where
    I: Read,
{
    dir: PathBuf,
    env: HashMap<String, String>,
    argv: VecDeque<String>,
    stdin: I,
    repo: Repository,
}

impl<I> CommandContext<I>
where
    I: Read,
{
    pub fn new(
        dir: PathBuf,
        env: HashMap<String, String>,
        argv: VecDeque<String>,
        stdin: I,
    ) -> Self {
        let repo = Repository::new(dir.join(".git"));

        Self {
            dir,
            env,
            argv,
            stdin,
            repo,
        }
    }
}
