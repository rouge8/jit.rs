use crate::errors::{Error, Result};
use crate::repository::Repository;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

mod add;
mod commit;
mod init;
mod status;

use add::Add;
use commit::Commit;
use init::Init;
use status::Status;

pub fn execute(
    dir: PathBuf,
    env: HashMap<String, String>,
    mut argv: VecDeque<String>,
) -> Result<()> {
    let name = if let Some(name) = argv.pop_front() {
        name
    } else {
        String::from("")
    };

    let ctx = CommandContext::new(dir, env, argv);

    match name.as_str() {
        "init" => Init::run(ctx),
        "add" => Add::run(ctx),
        "commit" => {
            let mut cmd = Commit::new(ctx);
            cmd.run()
        }
        "status" => {
            let mut cmd = Status::new(ctx);
            cmd.run()
        }
        _ => Err(Error::UnknownCommand(name.to_string())),
    }
}

pub struct CommandContext {
    dir: PathBuf,
    env: HashMap<String, String>,
    argv: VecDeque<String>,
    repo: Repository,
}

impl CommandContext {
    pub fn new(dir: PathBuf, env: HashMap<String, String>, argv: VecDeque<String>) -> Self {
        let repo = Repository::new(dir.join(".git"));

        Self {
            dir,
            env,
            argv,
            repo,
        }
    }
}
