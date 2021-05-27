use crate::errors::Result;
use std::env;
use std::fs;

pub struct Init;

impl Init {
    pub fn run() -> Result<()> {
        let args: Vec<String> = env::args().collect();

        let cwd = env::current_dir()?;
        let root_path = if let Some(path) = args.get(2) {
            cwd.join(path)
        } else {
            cwd
        };

        let git_path = root_path.join(".git");

        for dir in ["objects", "refs"].iter() {
            fs::create_dir_all(git_path.join(dir))?;
        }

        println!("Initialized empty Jit repository in {:?}", git_path);

        Ok(())
    }
}
