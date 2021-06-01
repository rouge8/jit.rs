use crate::errors::{Error, Result};
use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::path::PathBuf;

mod add;
mod commit;
mod init;

use add::Add;
use commit::Commit;
use init::Init;

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

    let command = match name.as_str() {
        "init" => Init::run,
        "add" => Add::run,
        "commit" => Commit::run,
        _ => return Err(Error::UnknownCommand(name.to_string())),
    };

    command(dir, env, argv, stdin)
}
