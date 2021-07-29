use crate::commands::{Command, CommandContext};
use crate::errors::Result;
use crate::util::path_to_string;
use std::path::{Path, PathBuf};

pub struct Rm<'a> {
    ctx: CommandContext<'a>,
    /// `jit rm <paths>...`
    paths: Vec<PathBuf>,
}

impl<'a> Rm<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Self {
        let paths = match &ctx.opt.cmd {
            Command::Rm { files } => files.to_owned(),
            _ => unreachable!(),
        };

        Self { ctx, paths }
    }

    pub fn run(&mut self) -> Result<()> {
        self.ctx.repo.index.load_for_update()?;
        let paths = self.paths.clone();
        for path in &paths {
            self.remove_file(path)?;
        }
        self.ctx.repo.index.write_updates()?;

        Ok(())
    }

    fn remove_file(&mut self, path: &Path) -> Result<()> {
        self.ctx.repo.index.remove(path);
        self.ctx.repo.workspace.remove(path)?;

        let mut stdout = self.ctx.stdout.borrow_mut();
        writeln!(stdout, "rm '{}'", path_to_string(path))?;

        Ok(())
    }
}
