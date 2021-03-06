use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Parser;

use crate::config::VariableValue;
use crate::editor::Editor;
use crate::errors::Result;
use crate::pager::Pager;
use crate::repository::Repository;

mod add;
mod branch;
mod checkout;
mod cherry_pick;
mod commit;
mod config;
mod diff;
mod init;
mod log;
mod merge;
mod remote;
mod reset;
mod revert;
mod rm;
mod shared;
mod status;

use add::Add;
use branch::Branch;
use checkout::Checkout;
use cherry_pick::CherryPick;
use commit::Commit;
use config::ConfigCommand as Config;
use diff::Diff;
use init::Init;
use log::{Log, LogDecoration, LogFormat};
use merge::Merge;
use remote::Remote;
use reset::Reset;
use revert::Revert;
use rm::Rm;
use status::Status;

#[derive(Parser, Debug)]
pub struct Jit {
    #[clap(subcommand)]
    pub cmd: Command,
}

#[derive(Parser, Debug)]
pub enum Command {
    Add {
        #[clap(value_parser)]
        files: Vec<PathBuf>,
    },
    Branch {
        args: Vec<String>,
        #[clap(short, long)]
        verbose: bool,
        #[clap(short, long)]
        delete: bool,
        #[clap(short, long)]
        force: bool,
        #[clap(short = 'D')]
        force_delete: bool,
    },
    Checkout {
        tree_ish: String,
    },
    CherryPick {
        args: Vec<String>,
        #[clap(long)]
        r#continue: bool,
        #[clap(long)]
        abort: bool,
        #[clap(long)]
        quit: bool,
        #[clap(short, long)]
        mainline: Option<u32>,
    },
    Commit {
        #[clap(short, long)]
        message: Option<String>,
        #[clap(short = 'F', long)]
        file: Option<PathBuf>,
        #[clap(long)]
        edit: bool,
        #[clap(long, overrides_with = "edit")]
        no_edit: bool,
        #[clap(short = 'C', long)]
        reuse_message: Option<String>,
        #[clap(short = 'c', long)]
        reedit_message: Option<String>,
        #[clap(long)]
        amend: bool,
    },
    Config {
        args: Vec<String>,
        #[clap(long)]
        local: bool,
        #[clap(long)]
        global: bool,
        #[clap(long)]
        system: bool,
        #[clap(short, long)]
        file: Option<PathBuf>,
        #[clap(long)]
        add: Option<String>,
        #[clap(long)]
        replace_all: Option<String>,
        #[clap(long)]
        get_all: Option<String>,
        #[clap(long)]
        unset: Option<String>,
        #[clap(long)]
        unset_all: Option<String>,
        #[clap(long)]
        remove_section: Option<String>,
    },
    Diff {
        args: Vec<String>,
        #[clap(long)]
        cached: bool,
        #[clap(long)]
        staged: bool,
        #[clap(short, long)]
        patch: bool,
        #[clap(short = 's', long)]
        no_patch: bool,
        #[clap(flatten)]
        stage: StageOptions,
    },
    Init {
        #[clap(value_parser)]
        directory: Option<PathBuf>,
    },
    Log {
        args: Vec<String>,
        #[clap(long = "abbrev-commit")]
        abbrev: bool,
        #[clap(long = "no-abbrev-commit", overrides_with = "abbrev", hide = true)]
        no_abbrev: bool,
        #[clap(arg_enum, long, visible_alias = "pretty", default_value = "medium")]
        format: LogFormat,
        #[clap(long = "oneline")]
        one_line: bool,
        /// The default option, if using `--decorate` alone is `short`.  If `--decorate` is not
        /// used, the default is `auto`. Otherwise, the value of `--decorate=<format>` is used.
        #[clap(arg_enum, long, value_name = "format")]
        #[allow(clippy::option_option)]
        decorate: Option<Option<LogDecoration>>,
        #[clap(long)]
        no_decorate: bool,
        #[clap(short, long)]
        patch: bool,
        #[clap(short = 's', long, overrides_with = "patch")]
        _no_patch: bool,
        #[clap(long = "cc")]
        combined: bool,
    },
    Merge {
        args: Vec<String>,
        #[clap(long)]
        abort: bool,
        #[clap(long)]
        r#continue: bool,
        #[clap(short, long)]
        message: Option<String>,
        #[clap(short = 'F', long)]
        file: Option<PathBuf>,
        #[clap(short, long)]
        #[clap(long)]
        edit: bool,
        #[clap(long, overrides_with = "edit")]
        no_edit: bool,
    },
    Remote {
        args: Vec<String>,
        #[clap(short, long)]
        verbose: bool,
        #[clap(short)]
        tracked: Vec<String>,
    },
    Reset {
        #[clap(value_parser)]
        files: Vec<PathBuf>,
        #[clap(long)]
        soft: bool,
        #[clap(long)]
        _mixed: bool,
        #[clap(long)]
        hard: bool,
    },
    Revert {
        args: Vec<String>,
        #[clap(long)]
        r#continue: bool,
        #[clap(long)]
        abort: bool,
        #[clap(long)]
        quit: bool,
        #[clap(short, long)]
        mainline: Option<u32>,
    },
    Rm {
        #[clap(value_parser)]
        files: Vec<PathBuf>,
        #[clap(long)]
        cached: bool,
        #[clap(short, long)]
        force: bool,
        #[clap(short)]
        recursive: bool,
    },
    Status {
        #[clap(long)]
        porcelain: bool,
    },
}

#[derive(Parser, Debug)]
pub struct StageOptions {
    #[clap(short = '1', long, group = "stage")]
    pub base: bool,
    #[clap(short = '2', long, group = "stage")]
    pub ours: bool,
    #[clap(short = '3', long, group = "stage")]
    pub theirs: bool,
}

pub fn execute<O: Write + 'static, E: Write + 'static>(
    dir: PathBuf,
    env: HashMap<String, String>,
    opt: Jit,
    stdout: O,
    stderr: E,
    isatty: bool,
) -> Result<()> {
    let ctx = CommandContext::new(dir, env, &opt, Box::new(stdout), Box::new(stderr), isatty);

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
        Command::CherryPick { .. } => {
            let mut cmd = CherryPick::new(ctx);
            cmd.run()
        }
        Command::Commit { .. } => {
            let mut cmd = Commit::new(ctx);
            cmd.run()
        }
        Command::Config { .. } => {
            let mut cmd = Config::new(ctx);
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
        Command::Merge { .. } => {
            let mut cmd = Merge::new(ctx)?;
            cmd.run()
        }
        Command::Remote { .. } => {
            let mut cmd = Remote::new(ctx);
            cmd.run()
        }
        Command::Reset { .. } => {
            let mut cmd = Reset::new(ctx)?;
            cmd.run()
        }
        Command::Revert { .. } => {
            let mut cmd = Revert::new(ctx);
            cmd.run()
        }
        Command::Rm { .. } => {
            let mut cmd = Rm::new(ctx)?;
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
    repo: Box<Repository>,
    stdout: RefCell<Box<dyn Write>>,
    stderr: RefCell<Box<dyn Write>>,
    using_pager: bool,
    pub isatty: bool,
}

impl<'a> CommandContext<'a> {
    pub fn new(
        dir: PathBuf,
        env: HashMap<String, String>,
        opt: &'a Jit,
        stdout: Box<dyn Write>,
        stderr: Box<dyn Write>,
        isatty: bool,
    ) -> Self {
        let repo = Box::new(Repository::new(dir.join(".git")));

        Self {
            dir,
            env,
            opt,
            repo,
            stdout: RefCell::new(stdout),
            stderr: RefCell::new(stderr),
            using_pager: false,
            isatty,
        }
    }

    pub fn setup_pager(&mut self) {
        // Only setup the pager once
        if self.using_pager {
            return;
        }

        // Only setup the pager if stdout is a tty
        if !self.isatty {
            return;
        }

        self.stdout = RefCell::new(Box::new(Pager::new(&self.env)));
        self.using_pager = true;
    }

    pub fn edit_file<F>(&self, path: &Path, f: F) -> Result<Option<String>>
    where
        F: Fn(&mut Editor) -> Result<()>,
    {
        Editor::edit(
            path.to_path_buf(),
            self.editor_command(),
            |editor: &mut Editor| {
                f(editor)?;
                if !self.isatty {
                    editor.close();
                }

                Ok(())
            },
        )
    }

    fn editor_command(&self) -> Option<String> {
        if let Some(editor) = self.env.get("GIT_EDITOR") {
            Some(editor.to_owned())
        } else if let Some(editor) = self
            .repo
            .config
            .get(&[String::from("core"), String::from("editor")])
        {
            match editor {
                VariableValue::String(editor) => Some(editor),
                _ => unimplemented!(),
            }
        } else if let Some(editor) = self.env.get("VISUAL") {
            Some(editor.to_owned())
        } else {
            self.env.get("EDITOR").map(|editor| editor.to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_app() {
        use clap::IntoApp;

        Jit::command().debug_assert()
    }
}
