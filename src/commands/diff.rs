use crate::commands::{Command, CommandContext};
use crate::database::blob::Blob;
use crate::database::{Database, ParsedObject};
use crate::diff::hunk::Hunk;
use crate::diff::{diff_hunks, Edit, EditType};
use crate::errors::Result;
use crate::index::Entry;
use crate::repository::ChangeType;
use colored::Colorize;
use lazy_static::lazy_static;
use std::cell::RefMut;
use std::io::Write;
use std::path::Path;

lazy_static! {
    static ref NULL_OID: String = "0".repeat(40);
}
const NULL_PATH: &str = "/dev/null";

pub struct Diff<'a> {
    ctx: CommandContext<'a>,
    /// `jit diff --cached` or `jit diff --staged`
    cached: bool,
}

impl<'a> Diff<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Self {
        let cached = match ctx.opt.cmd {
            Command::Diff { cached, staged } => cached || staged,
            _ => unreachable!(),
        };

        Self { ctx, cached }
    }

    pub fn run(&mut self) -> Result<()> {
        self.ctx.repo.index.load()?;
        self.ctx.repo.initialize_status()?;

        self.ctx.setup_pager();

        if self.cached {
            self.diff_head_index()?;
        } else {
            self.diff_index_workspace()?;
        }

        Ok(())
    }

    fn diff_head_index(&mut self) -> Result<()> {
        let paths: Vec<_> = self.ctx.repo.index_changes.keys().cloned().collect();
        for path in paths {
            let state = &self.ctx.repo.index_changes[&path];
            match state {
                ChangeType::Added => {
                    let mut a = self.from_nothing(&path);
                    let mut b = self.from_index(&path)?;

                    self.print_diff(&mut a, &mut b)?;
                }
                ChangeType::Modified => {
                    let mut a = self.from_head(&path)?;
                    let mut b = self.from_index(&path)?;

                    self.print_diff(&mut a, &mut b)?;
                }
                ChangeType::Deleted => {
                    let mut a = self.from_head(&path)?;
                    let mut b = self.from_nothing(&path);

                    self.print_diff(&mut a, &mut b)?;
                }
                ChangeType::Untracked => unreachable!(),
            }
        }

        Ok(())
    }

    fn diff_index_workspace(&mut self) -> Result<()> {
        let paths: Vec<_> = self.ctx.repo.workspace_changes.keys().cloned().collect();
        for path in paths {
            let state = &self.ctx.repo.workspace_changes[&path];
            match state {
                ChangeType::Modified => {
                    let mut a = self.from_index(&path)?;
                    let mut b = self.from_file(&path)?;

                    self.print_diff(&mut a, &mut b)?;
                }
                ChangeType::Deleted => {
                    let mut a = self.from_index(&path)?;
                    let mut b = self.from_nothing(&path);

                    self.print_diff(&mut a, &mut b)?;
                }
                _ => unreachable!(),
            }
        }

        Ok(())
    }

    fn from_head(&mut self, path: &str) -> Result<Target> {
        let entry = &self.ctx.repo.head_tree[path];
        let oid = entry.oid();
        let blob = match self.ctx.repo.database.load(&oid)? {
            ParsedObject::Blob(blob) => blob,
            _ => unreachable!(),
        };

        Ok(Target::new(
            path.to_string(),
            oid,
            Some(entry.mode()),
            blob.data.clone(),
        ))
    }

    fn from_index(&mut self, path: &str) -> Result<Target> {
        let entry = self.ctx.repo.index.entry_for_path(path).unwrap();
        let blob = match self.ctx.repo.database.load(&entry.oid)? {
            ParsedObject::Blob(blob) => blob,
            _ => unreachable!(),
        };

        Ok(Target::new(
            path.to_string(),
            entry.oid.clone(),
            Some(entry.mode),
            blob.data.clone(),
        ))
    }

    fn from_file(&self, path: &str) -> Result<Target> {
        let blob = Blob::new(self.ctx.repo.workspace.read_file(Path::new(path))?);
        let oid = self.ctx.repo.database.hash_object(&blob);
        let mode = Entry::mode_for_stat(&self.ctx.repo.stats[path]);

        Ok(Target::new(path.to_string(), oid, Some(mode), blob.data))
    }

    fn from_nothing(&self, path: &str) -> Target {
        Target::new(path.to_string(), NULL_OID.to_string(), None, vec![])
    }

    fn short(&self, oid: &str) -> String {
        Database::short_oid(oid)
    }

    fn print_diff(&self, a: &mut Target, b: &mut Target) -> Result<()> {
        if a.oid == b.oid && a.mode == b.mode {
            return Ok(());
        }

        a.path = format!("a/{}", a.path);
        b.path = format!("b/{}", b.path);

        let mut stdout = self.ctx.stdout.borrow_mut();
        writeln!(stdout, "diff --git {} {}", a.path, b.path)?;
        self.print_diff_mode(&mut stdout, &a, &b)?;
        self.print_diff_content(&mut stdout, &a, &b)?;

        Ok(())
    }

    fn header(&self, stdout: &mut RefMut<Box<dyn Write>>, string: String) -> Result<()> {
        writeln!(stdout, "{}", string.bold())?;

        Ok(())
    }

    fn print_diff_mode(
        &self,
        stdout: &mut RefMut<Box<dyn Write>>,
        a: &Target,
        b: &Target,
    ) -> Result<()> {
        if a.mode.is_none() {
            self.header(stdout, format!("new file mode {:o}", b.mode.unwrap()))?;
        } else if b.mode.is_none() {
            self.header(stdout, format!("deleted file mode {:o}", a.mode.unwrap()))?;
        } else if a.mode != b.mode {
            self.header(stdout, format!("old mode {:o}", a.mode.unwrap()))?;
            self.header(stdout, format!("new mode {:o}", b.mode.unwrap()))?;
        }

        Ok(())
    }

    fn print_diff_content(
        &self,
        stdout: &mut RefMut<Box<dyn Write>>,
        a: &Target,
        b: &Target,
    ) -> Result<()> {
        if a.oid == b.oid {
            return Ok(());
        }

        let mut oid_range = format!("index {}..{}", self.short(&a.oid), self.short(&b.oid));
        if a.mode == b.mode {
            oid_range.push(' ');
            oid_range.push_str(&format!("{:o}", a.mode.unwrap()));
        }

        writeln!(stdout, "{}", oid_range)?;
        writeln!(stdout, "--- {}", a.diff_path())?;
        writeln!(stdout, "+++ {}", b.diff_path())?;

        let hunks = diff_hunks(
            std::str::from_utf8(&a.data).expect("Invalid UTF-8"),
            std::str::from_utf8(&b.data).expect("Invalid UTF-8"),
        );
        for hunk in hunks {
            self.print_diff_hunk(stdout, &hunk)?;
        }

        Ok(())
    }

    fn print_diff_hunk(&self, stdout: &mut RefMut<Box<dyn Write>>, hunk: &Hunk) -> Result<()> {
        writeln!(stdout, "{}", hunk.header().cyan())?;
        for edit in &hunk.edits {
            self.print_diff_edit(stdout, &edit)?;
        }

        Ok(())
    }

    fn print_diff_edit(&self, stdout: &mut RefMut<Box<dyn Write>>, edit: &Edit) -> Result<()> {
        let text = edit.to_string();

        match edit.r#type {
            EditType::Eql => writeln!(stdout, "{}", text)?,
            EditType::Ins => writeln!(stdout, "{}", text.green())?,
            EditType::Del => writeln!(stdout, "{}", text.red())?,
        }

        Ok(())
    }
}

struct Target {
    path: String,
    oid: String,
    mode: Option<u32>,
    data: Vec<u8>,
}

impl Target {
    fn new(path: String, oid: String, mode: Option<u32>, data: Vec<u8>) -> Self {
        Target {
            path,
            oid,
            mode,
            data,
        }
    }

    fn diff_path(&self) -> &str {
        match self.mode {
            Some(_) => &self.path,
            None => NULL_PATH,
        }
    }
}
