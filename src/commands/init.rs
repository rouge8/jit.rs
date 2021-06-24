use crate::commands::CommandContext;
use crate::errors::Result;
use crate::refs::Refs;
use std::fs;
use std::io::Write;

const DEFAULT_BRANCH: &str = "main";

pub struct Init<E: Write> {
    ctx: CommandContext<E>,
}

impl<E: Write> Init<E> {
    pub fn new(ctx: CommandContext<E>) -> Self {
        Self { ctx }
    }

    pub fn run(&self) -> Result<()> {
        let root_path = if let Some(path) = self.ctx.argv.get(0) {
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
