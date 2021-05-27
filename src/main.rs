use anyhow::Result;
use std::env;
use std::process;

mod commands;
mod database;
mod errors;
mod index;
mod lockfile;
mod refs;
mod repository;
mod util;
mod workspace;
use errors::Error;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    let command = if let Some(command) = args.get(1) {
        command.as_str()
    } else {
        ""
    };

    match commands::execute(&command) {
        Ok(()) => (),
        Err(err) => match err {
            Error::UnknownCommand(..) => {
                eprintln!("jit: {}", err);
                process::exit(1);
            }
            _ => {
                eprintln!("fatal: {}", err);
                process::exit(1);
            }
        },
    }

    Ok(())
}
