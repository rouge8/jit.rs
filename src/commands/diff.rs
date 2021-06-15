use crate::commands::CommandContext;
use crate::database::blob::Blob;
use crate::database::ParsedObject;
use crate::diff::hunk::Hunk;
use crate::diff::{diff_hunks, Edit, EditType};
use crate::errors::Result;
use crate::index::Entry;
use crate::repository::{ChangeType, Repository};
use colored::Colorize;
use lazy_static::lazy_static;
use std::collections::VecDeque;
use std::path::Path;

lazy_static! {
    static ref NULL_OID: String = "0".repeat(40);
}
const NULL_PATH: &str = "/dev/null";

pub struct Diff {
    repo: Repository,
    argv: VecDeque<String>,
}

impl Diff {
    pub fn new(ctx: CommandContext) -> Self {
        Self {
            repo: ctx.repo,
            argv: ctx.argv,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        self.repo.index.load()?;
        self.repo.initialize_status()?;

        if self.argv.contains(&String::from("--cached")) {
            self.diff_head_index()?;
        } else {
            self.diff_index_workspace()?;
        }

        Ok(())
    }

    fn diff_head_index(&mut self) -> Result<()> {
        let paths: Vec<_> = self.repo.index_changes.keys().cloned().collect();
        for path in paths {
            let state = &self.repo.index_changes[&path];
            match state {
                ChangeType::Added => {
                    let mut a = self.from_nothing(&path);
                    let mut b = self.from_index(&path)?;

                    self.print_diff(&mut a, &mut b);
                }
                ChangeType::Modified => {
                    let mut a = self.from_head(&path)?;
                    let mut b = self.from_index(&path)?;

                    self.print_diff(&mut a, &mut b);
                }
                ChangeType::Deleted => {
                    let mut a = self.from_head(&path)?;
                    let mut b = self.from_nothing(&path);

                    self.print_diff(&mut a, &mut b);
                }
            }
        }

        Ok(())
    }

    fn diff_index_workspace(&mut self) -> Result<()> {
        let paths: Vec<_> = self.repo.workspace_changes.keys().cloned().collect();
        for path in paths {
            let state = &self.repo.workspace_changes[&path];
            match state {
                ChangeType::Modified => {
                    let mut a = self.from_index(&path)?;
                    let mut b = self.from_file(&path)?;

                    self.print_diff(&mut a, &mut b);
                }
                ChangeType::Deleted => {
                    let mut a = self.from_index(&path)?;
                    let mut b = self.from_nothing(&path);

                    self.print_diff(&mut a, &mut b);
                }
                _ => unreachable!(),
            }
        }

        Ok(())
    }

    fn from_head(&mut self, path: &str) -> Result<Target> {
        let entry = &self.repo.head_tree[path];
        let oid = entry.oid();
        let blob = match self.repo.database.load(&oid)? {
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
        let entry = self.repo.index.entry_for_path(path);
        let blob = match self.repo.database.load(&entry.oid)? {
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
        let blob = Blob::new(self.repo.workspace.read_file(Path::new(path))?);
        let oid = self.repo.database.hash_object(&blob);
        let mode = Entry::mode_for_stat(&self.repo.stats[path]);

        Ok(Target::new(path.to_string(), oid, Some(mode), blob.data))
    }

    fn from_nothing(&self, path: &str) -> Target {
        Target::new(path.to_string(), NULL_OID.to_string(), None, vec![])
    }

    fn short(&self, oid: &str) -> String {
        self.repo.database.short_oid(oid)
    }

    fn print_diff(&mut self, a: &mut Target, b: &mut Target) {
        if a.oid == b.oid && a.mode == b.mode {
            return;
        }

        a.path = format!("a/{}", a.path);
        b.path = format!("b/{}", b.path);

        println!("diff --git {} {}", a.path, b.path);
        self.print_diff_mode(&a, &b);
        self.print_diff_content(&a, &b);
    }

    fn header(&self, string: String) {
        println!("{}", string.bold());
    }

    fn print_diff_mode(&self, a: &Target, b: &Target) {
        if a.mode.is_none() {
            self.header(format!("new file mode {:o}", b.mode.unwrap()));
        } else if b.mode.is_none() {
            self.header(format!("deleted file mode {:o}", a.mode.unwrap()));
        } else if a.mode != b.mode {
            self.header(format!("old mode {:o}", a.mode.unwrap()));
            self.header(format!("new mode {:o}", b.mode.unwrap()));
        }
    }

    fn print_diff_content(&self, a: &Target, b: &Target) {
        if a.oid == b.oid {
            return;
        }

        let mut oid_range = format!("index {}..{}", self.short(&a.oid), self.short(&b.oid));
        if a.mode == b.mode {
            oid_range.push(' ');
            oid_range.push_str(&format!("{:o}", a.mode.unwrap()));
        }

        println!("{}", oid_range);
        println!("--- {}", a.diff_path());
        println!("+++ {}", b.diff_path());

        let hunks = diff_hunks(
            std::str::from_utf8(&a.data).expect("Invalid UTF-8"),
            std::str::from_utf8(&b.data).expect("Invalid UTF-8"),
        );
        for hunk in hunks {
            self.print_diff_hunk(&hunk);
        }
    }

    fn print_diff_hunk(&self, hunk: &Hunk) {
        println!("{}", hunk.header().cyan());
        for edit in &hunk.edits {
            self.print_diff_edit(&edit);
        }
    }

    fn print_diff_edit(&self, edit: &Edit) {
        let text = edit.to_string();

        match edit.r#type {
            EditType::Eql => println!("{}", text),
            EditType::Ins => println!("{}", text.green()),
            EditType::Del => println!("{}", text.red()),
        }
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
