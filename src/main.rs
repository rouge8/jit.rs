use anyhow::Result;
use jit::commands;
use jit::errors::Error;
use std::collections::HashMap;
use std::env;
use std::io;
use std::process;
use structopt::StructOpt;

fn main() -> Result<()> {
    let opt = commands::Jit::from_args();

    match commands::execute(
        env::current_dir()?,
        env::vars().collect::<HashMap<String, String>>(),
        opt,
        io::stdout(),
        io::stderr(),
        atty::is(atty::Stream::Stdout),
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
            Error::Io(err) => {
                if err.kind() == io::ErrorKind::BrokenPipe {
                    // Suppress "broken pipe" error messages
                    //
                    // We see these when using the pager and exiting early or piping the output to
                    // another process like `head`.
                    // ref: https://github.com/rust-lang/rust/issues/46016
                    process::exit(0);
                } else {
                    eprintln!("fatal: {}", err);
                    process::exit(1);
                }
            }
            _ => {
                eprintln!("fatal: {}", err);
                process::exit(1);
            }
        },
    }

    Ok(())
}
