use crate::commands::CommandContext;
use crate::errors::Result;
use std::fs;
use std::io::Read;

pub struct Init;

impl Init {
    pub fn run<I: Read>(ctx: CommandContext<I>) -> Result<()> {
        let root_path = if let Some(path) = ctx.argv.get(1) {
            ctx.dir.join(path)
        } else {
            ctx.dir
        };

        let git_path = root_path.join(".git");

        for dir in ["objects", "refs"].iter() {
            fs::create_dir_all(git_path.join(dir))?;
        }

        println!("Initialized empty Jit repository in {:?}", git_path);

        Ok(())
    }
}
