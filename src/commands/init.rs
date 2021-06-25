use crate::commands::{Command, CommandContext};
use crate::errors::Result;
use crate::refs::Refs;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

const DEFAULT_BRANCH: &str = "main";

pub struct Init<'a> {
    ctx: CommandContext<'a>,
    /// `jit init <directory>`
    directory: Option<PathBuf>,
}

impl<'a> Init<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Self {
        let directory = match &ctx.opt.cmd {
            Command::Init { directory } => directory.to_owned(),
            _ => unreachable!(),
        };

        Self { ctx, directory }
    }

    pub fn run(&self) -> Result<()> {
        let root_path = if let Some(path) = &self.directory {
            self.ctx.dir.join(path)
        } else {
            self.ctx.dir.clone()
        };

        let git_path = root_path.join(".git");

        for dir in ["objects", "refs/heads"].iter() {
            fs::create_dir_all(git_path.join(dir))?;
        }

        let refs = Refs::new(git_path.clone());
        let path = format!("refs/heads/{}", DEFAULT_BRANCH);
        refs.update_head(format!("ref: {}", path))?;

        let mut stdout = self.ctx.stdout.borrow_mut();
        writeln!(stdout, "Initialized empty Jit repository in {:?}", git_path)?;

        Ok(())
    }
}
