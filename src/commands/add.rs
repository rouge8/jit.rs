use std::io;
use std::io::Write;
use std::path::PathBuf;

use crate::commands::{Command, CommandContext};
use crate::database::blob::Blob;
use crate::database::object::Object;
use crate::errors::{Error, Result};
use crate::util::path_to_string;

pub struct Add<'a> {
    ctx: CommandContext<'a>,
    /// `jit add <paths>...`
    paths: Vec<PathBuf>,
}

impl<'a> Add<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Self {
        let paths = match &ctx.opt.cmd {
            Command::Add { files } => files.to_owned(),
            _ => unreachable!(),
        };

        Self { ctx, paths }
    }

    pub fn run(&mut self) -> Result<()> {
        if self.paths.is_empty() {
            let mut stderr = self.ctx.stderr.borrow_mut();
            writeln!(stderr, "Nothing specified, nothing added.")?;
            return Err(Error::Exit(0));
        }

        match self.ctx.repo.index.load_for_update() {
            Ok(()) => (),
            Err(err) => return self.handle_locked_index(err),
        }

        let paths = self.paths.clone();
        for path in &paths {
            let path = match path.canonicalize() {
                Ok(path) => path,
                Err(err) => return self.handle_missing_file(&path_to_string(path), err),
            };

            for path in self.ctx.repo.workspace.list_files(&path)? {
                self.add_to_index(path)?;
            }
        }

        self.ctx.repo.index.write_updates()?;

        Ok(())
    }

    fn add_to_index(&mut self, path: PathBuf) -> Result<()> {
        let data = match self.ctx.repo.workspace.read_file(&path) {
            Ok(data) => data,
            Err(err) => return self.handle_unreadable_file(err),
        };
        let stat = match self.ctx.repo.workspace.stat_file(&path) {
            Ok(stat) => stat.unwrap(),
            Err(err) => return self.handle_unreadable_file(err),
        };

        let blob = Blob::new(data);
        self.ctx.repo.database.store(&blob)?;
        self.ctx.repo.index.add(path, blob.oid(), stat);

        Ok(())
    }

    fn handle_locked_index(&self, err: Error) -> Result<()> {
        let mut stderr = self.ctx.stderr.borrow_mut();
        match err {
            Error::LockDenied(..) => {
                writeln!(stderr, "fatal: {}", err)?;
                writeln!(
                    stderr,
                    "
Another jit process seems to be running in this repository.
Please make sure all processes are terminated then try again.
If it still fails, a jit process may have crashed in this
repository earlier: remove the file manually to continue."
                )?;
                Err(Error::Exit(128))
            }
            _ => Err(err),
        }
    }

    fn handle_missing_file(&mut self, path: &str, err: io::Error) -> Result<()> {
        let mut stderr = self.ctx.stderr.borrow_mut();
        if err.kind() == io::ErrorKind::NotFound {
            writeln!(stderr, "fatal: pathspec '{}' did not match any files", path)?;
            self.ctx.repo.index.release_lock()?;
            Err(Error::Exit(128))
        } else {
            Err(Error::Io(err))
        }
    }

    fn handle_unreadable_file(&mut self, err: Error) -> Result<()> {
        let mut stderr = self.ctx.stderr.borrow_mut();
        match err {
            Error::NoPermission { .. } => {
                writeln!(stderr, "error: {}", err)?;
                writeln!(stderr, "fatal: adding files failed")?;
                self.ctx.repo.index.release_lock()?;
                Err(Error::Exit(128))
            }
            _ => Err(err),
        }
    }
}
