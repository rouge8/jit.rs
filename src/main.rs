use anyhow::Result;
use std::collections::{HashMap, VecDeque};
use std::env;
use std::io;
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
    let mut argv: VecDeque<String> = env::args().collect();

    // Remove the executable name from argv
    argv.pop_front();

    match commands::execute(
        env::current_dir()?,
        env::vars().collect::<HashMap<String, String>>(),
        argv,
        io::stdin(),
        io::stdout(),
        io::stderr(),
    ) {
        Ok(()) => (),
        Err(err) => match err {
            Error::UnknownCommand(..) => {
                eprintln!("jit: {}", err);
                process::exit(1);
            }
            Error::Exit(code) => {
                process::exit(code);
            }
            _ => {
                eprintln!("fatal: {}", err);
                process::exit(1);
            }
        },
    }

    Ok(())
}
