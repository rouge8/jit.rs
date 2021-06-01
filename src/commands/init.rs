use crate::errors::Result;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::Read;
use std::path::PathBuf;

pub struct Init;

impl Init {
    pub fn run<I: Read>(
        dir: PathBuf,
        _env: HashMap<String, String>,
        argv: VecDeque<String>,
        _stdin: I,
    ) -> Result<()> {
        let root_path = if let Some(path) = argv.get(1) {
            dir.join(path)
        } else {
            dir
        };

        let git_path = root_path.join(".git");

        for dir in ["objects", "refs"].iter() {
            fs::create_dir_all(git_path.join(dir))?;
        }

        println!("Initialized empty Jit repository in {:?}", git_path);

        Ok(())
    }
}
