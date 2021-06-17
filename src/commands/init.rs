use crate::commands::CommandContext;
use crate::errors::Result;
use std::fs;
use std::io::Write;

pub struct Init<E: Write> {
    ctx: CommandContext<E>,
}

impl<E: Write> Init<E> {
    pub fn new(ctx: CommandContext<E>) -> Self {
        Self { ctx }
    }

    pub fn run(&self) -> Result<()> {
        let root_path = if let Some(path) = self.ctx.argv.get(1) {
            self.ctx.dir.join(path)
        } else {
            self.ctx.dir.clone()
        };

        let git_path = root_path.join(".git");

        for dir in ["objects", "refs"].iter() {
            fs::create_dir_all(git_path.join(dir))?;
        }

        let mut stdout = self.ctx.stdout.borrow_mut();
        writeln!(stdout, "Initialized empty Jit repository in {:?}", git_path)?;

        Ok(())
    }
}
