use crate::errors::Result;
use crate::pager::Pager;
use crate::repository::Repository;
use atty::Stream;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use structopt::StructOpt;

mod add;
mod branch;
mod checkout;
mod commit;
mod diff;
mod init;
mod log;
mod status;

use add::Add;
use branch::Branch;
use checkout::Checkout;
use commit::Commit;
use diff::Diff;
use init::Init;
use log::Log;
use status::Status;

#[derive(StructOpt, Debug)]
pub struct Jit {
    #[structopt(subcommand)]
    pub cmd: Command,
}

#[derive(StructOpt, Debug)]
pub enum Command {
    Add {
        #[structopt(parse(from_os_str))]
        files: Vec<PathBuf>,
    },
    Branch {
        args: Vec<String>,
        #[structopt(short, long)]
        verbose: bool,
        #[structopt(short, long)]
        delete: bool,
        #[structopt(short, long)]
        force: bool,
        #[structopt(short = "D")]
        force_delete: bool,
    },
    Checkout {
        tree_ish: String,
    },
    Commit,
    Diff {
        #[structopt(long)]
        cached: bool,
        #[structopt(long)]
        staged: bool,
    },
    Init {
        #[structopt(parse(from_os_str))]
        directory: Option<PathBuf>,
    },
    Log {
        #[structopt(long = "abbrev-commit")]
        abbrev: bool,
        #[structopt(long = "no-abbrev-commit", overrides_with = "abbrev", hidden = true)]
        _no_abbrev: bool,
    },
    Status {
        #[structopt(long)]
        porcelain: bool,
    },
}

pub fn execute<O: Write + 'static, E: Write + 'static>(
    dir: PathBuf,
    env: HashMap<String, String>,
    opt: Jit,
    stdout: O,
    stderr: E,
) -> Result<()> {
    let ctx = CommandContext::new(dir, env, &opt, Box::new(stdout), Box::new(stderr));

    match &opt.cmd {
        Command::Add { .. } => {
            let mut cmd = Add::new(ctx);
            cmd.run()
        }
        Command::Branch { .. } => {
            let mut cmd = Branch::new(ctx);
            cmd.run()
        }
        Command::Checkout { .. } => {
            let mut cmd = Checkout::new(ctx);
            cmd.run()
        }
        Command::Commit { .. } => {
            let mut cmd = Commit::new(ctx);
            cmd.run()
        }
        Command::Diff { .. } => {
            let mut cmd = Diff::new(ctx);
            cmd.run()
        }
        Command::Init { .. } => {
            let cmd = Init::new(ctx);
            cmd.run()
        }
        Command::Log { .. } => {
            let mut cmd = Log::new(ctx);
            cmd.run()
        }
        Command::Status { .. } => {
            let mut cmd = Status::new(ctx);
            cmd.run()
        }
    }
}

pub struct CommandContext<'a> {
    dir: PathBuf,
    env: HashMap<String, String>,
    opt: &'a Jit,
    repo: Repository,
    stdout: RefCell<Box<dyn Write>>,
    stderr: RefCell<Box<dyn Write>>,
    using_pager: bool,
}

impl<'a> CommandContext<'a> {
    pub fn new(
        dir: PathBuf,
        env: HashMap<String, String>,
        opt: &'a Jit,
        stdout: Box<dyn Write>,
        stderr: Box<dyn Write>,
    ) -> Self {
        let repo = Repository::new(dir.join(".git"));

        Self {
            dir,
            env,
            opt,
            repo,
            stdout: RefCell::new(stdout),
            stderr: RefCell::new(stderr),
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
