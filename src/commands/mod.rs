use crate::errors::{Error, Result};

mod add;
mod commit;
mod init;

use add::Add;
use commit::Commit;
use init::Init;

pub fn execute(name: &str) -> Result<()> {
    match name {
        "init" => Init::run()?,
        "add" => Add::run()?,
        "commit" => Commit::run()?,
        _ => return Err(Error::UnknownCommand(name.to_string())),
    }

    Ok(())
}
