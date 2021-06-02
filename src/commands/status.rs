use crate::commands::CommandContext;
use crate::errors::Result;
use crate::util::path_to_string;
use std::io::Read;

pub struct Status;

impl Status {
    pub fn run<I: Read>(mut ctx: CommandContext<I>) -> Result<()> {
        ctx.repo.index.load()?;

        let paths = ctx.repo.workspace.list_files(&ctx.dir)?;

        let mut untracked: Vec<_> = paths
            .iter()
            .filter(|path| !ctx.repo.index.tracked(&path))
            .collect();

        untracked.sort();

        for path in untracked {
            println!("?? {}", path_to_string(&path));
        }

        Ok(())
    }
}
