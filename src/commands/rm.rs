use crate::commands::{Command, CommandContext};
use crate::errors::{Error, Result};
use crate::util::path_to_string;
use std::path::{Path, PathBuf};

pub struct Rm<'a> {
    ctx: CommandContext<'a>,
    /// `jit rm <paths>...`
    paths: Vec<PathBuf>,
    head_oid: String,
    uncommitted: Vec<PathBuf>,
    unstaged: Vec<PathBuf>,
}

impl<'a> Rm<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Result<Self> {
        let paths = match &ctx.opt.cmd {
            Command::Rm { files } => files.to_owned(),
            _ => unreachable!(),
        };

        // TODO: Support running `jit rm` before the first commit by handling `head_oid = None`.
        let head_oid = ctx.repo.refs.read_head()?.unwrap();

        Ok(Self {
            ctx,
            paths,
            head_oid,
            uncommitted: Vec::new(),
            unstaged: Vec::new(),
        })
    }

    pub fn run(&mut self) -> Result<()> {
        self.ctx.repo.index.load_for_update()?;

        let paths = self.paths.clone();
        for path in &paths {
            self.plan_removal(path)?;
        }
        self.exit_on_errors()?;

        for path in &paths {
            self.remove_file(path)?;
        }
        self.ctx.repo.index.write_updates()?;

        Ok(())
    }

    fn plan_removal(&mut self, path: &Path) -> Result<()> {
        let item = self
            .ctx
            .repo
            .database
            .load_tree_entry(&self.head_oid, path)?;
        let entry = self.ctx.repo.index.entry_for_path(&path_to_string(path), 0);
        let stat = self.ctx.repo.workspace.stat_file(path)?;

        if self
            .ctx
            .repo
            .compare_tree_to_index(item.as_ref(), entry)
            .is_some()
        {
            self.uncommitted.push(path.to_path_buf());
        } else if stat.is_some()
            && self
                .ctx
                .repo
                .compare_index_to_workspace(entry, stat.as_ref())?
                .is_some()
        {
            self.unstaged.push(path.to_path_buf());
        }

        Ok(())
    }

    fn remove_file(&mut self, path: &Path) -> Result<()> {
        self.ctx.repo.index.remove(path);
        self.ctx.repo.workspace.remove(path)?;

        let mut stdout = self.ctx.stdout.borrow_mut();
        writeln!(stdout, "rm '{}'", path_to_string(path))?;

        Ok(())
    }

    fn exit_on_errors(&self) -> Result<()> {
        if self.uncommitted.is_empty() && self.unstaged.is_empty() {
            return Ok(());
        }

        self.print_errors(&self.uncommitted, "changes staged in the index")?;
        self.print_errors(&self.unstaged, "local modifications")?;

        Err(Error::Exit(1))
    }

    fn print_errors(&self, paths: &[PathBuf], message: &str) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }

        let files_have = if paths.len() == 1 {
            "file has"
        } else {
            "files have"
        };

        let mut stderr = self.ctx.stderr.borrow_mut();
        writeln!(stderr, "error: the following {} {}:", files_have, message,)?;
        for path in paths {
            writeln!(stderr, "    {}", path_to_string(path))?;
        }

        Ok(())
    }
}
