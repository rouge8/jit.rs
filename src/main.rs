use anyhow::Result;
use jit::commands;
use jit::errors::Error;
use std::collections::{HashMap, VecDeque};
use std::env;
use std::process;

fn main() -> Result<()> {
    let mut argv: VecDeque<String> = env::args().collect();

    // Remove the executable name from argv
    argv.pop_front();

    match commands::execute(
        env::current_dir()?,
        env::vars().collect::<HashMap<String, String>>(),
        argv,
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
