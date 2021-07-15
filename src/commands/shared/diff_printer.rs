use crate::database::entry::Entry;
use crate::database::tree_diff::Differ;
use crate::database::Database;
use crate::diff::hunk::Hunk;
use crate::diff::{diff_hunks, Edit, EditType};
use crate::errors::Result;
use crate::repository::Repository;
use crate::util::path_to_string;
use colored::Colorize;
use lazy_static::lazy_static;
use std::cell::RefMut;
use std::io::Write;

lazy_static! {
    static ref NULL_OID: String = "0".repeat(40);
}
const NULL_PATH: &str = "/dev/null";

pub struct Target {
    path: String,
    oid: String,
    mode: Option<u32>,
    data: Vec<u8>,
}

impl Target {
    pub fn new(path: String, oid: String, mode: Option<u32>, data: Vec<u8>) -> Self {
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

pub struct DiffPrinter {}

impl DiffPrinter {
    pub fn new() -> Self {
        Self {}
    }

    fn from_entry(&self, repo: &Repository, path: &str, entry: Option<&Entry>) -> Result<Target> {
        match entry {
            Some(entry) => {
                let blob = repo.database.load_blob(&entry.oid)?;

                Ok(Target::new(
                    path.to_string(),
                    entry.oid.clone(),
                    Some(entry.mode()),
                    blob.data,
                ))
            }
            None => Ok(self.from_nothing(&path)),
        }
    }

    pub fn from_nothing(&self, path: &str) -> Target {
        Target::new(path.to_string(), NULL_OID.to_string(), None, vec![])
    }

    fn header(&self, stdout: &mut RefMut<Box<dyn Write>>, string: String) -> Result<()> {
        writeln!(stdout, "{}", string.bold())?;

        Ok(())
    }

    fn short(&self, oid: &str) -> String {
        Database::short_oid(oid)
    }

    pub fn print_commit_diff(
        &self,
        stdout: &mut RefMut<Box<dyn Write>>,
        repo: &Repository,
        a: Option<&str>,
        b: &str,
        differ: Option<&dyn Differ>,
    ) -> Result<()> {
        let diff = if let Some(differ) = differ {
            differ.tree_diff(a, Some(b), None)?
        } else {
            repo.database.tree_diff(a, Some(b), None)?
        };
        let mut paths: Vec<_> = diff.keys().collect();
        paths.sort();

        for path in paths {
            let (old_entry, new_entry) = &diff[path];
            let path = path_to_string(&path);

            self.print_diff(
                stdout,
                &mut self.from_entry(&repo, &path, old_entry.as_ref())?,
                &mut self.from_entry(&repo, &path, new_entry.as_ref())?,
            )?;
        }

        Ok(())
    }

    pub fn print_diff(
        &self,
        stdout: &mut RefMut<Box<dyn Write>>,
        a: &mut Target,
        b: &mut Target,
    ) -> Result<()> {
        if a.oid == b.oid && a.mode == b.mode {
            return Ok(());
        }

        a.path = format!("a/{}", a.path);
        b.path = format!("b/{}", b.path);

        writeln!(stdout, "diff --git {} {}", a.path, b.path)?;
        self.print_diff_mode(stdout, &a, &b)?;
        self.print_diff_content(stdout, &a, &b)?;

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
