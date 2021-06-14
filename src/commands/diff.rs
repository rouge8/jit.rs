use crate::commands::CommandContext;
use crate::database::blob::Blob;
use crate::errors::Result;
use crate::index::Entry;
use crate::repository::{ChangeType, Repository};
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

    fn diff_head_index(&self) -> Result<()> {
        for (path, state) in &self.repo.index_changes {
            match state {
                ChangeType::Added => {
                    self.print_diff(&mut self.from_nothing(&path), &mut self.from_index(&path));
                }
                ChangeType::Modified => {
                    self.print_diff(&mut self.from_head(&path), &mut self.from_file(&path)?);
                }
                ChangeType::Deleted => {
                    self.print_diff(&mut self.from_head(&path), &mut self.from_nothing(&path));
                }
            }
        }

        Ok(())
    }

    fn diff_index_workspace(&self) -> Result<()> {
        for (path, state) in &self.repo.workspace_changes {
            match state {
                ChangeType::Modified => {
                    self.print_diff(&mut self.from_index(&path), &mut self.from_file(&path)?);
                }
                ChangeType::Deleted => {
                    self.print_diff(&mut self.from_index(&path), &mut self.from_nothing(&path));
                }
                _ => unreachable!(),
            }
        }

        Ok(())
    }

    fn from_head(&self, path: &str) -> Target {
        let entry = &self.repo.head_tree[path];

        Target::new(path.to_string(), entry.oid(), Some(entry.mode()))
    }

    fn from_index(&self, path: &str) -> Target {
        let entry = self.repo.index.entry_for_path(path);

        Target::new(path.to_string(), entry.oid.clone(), Some(entry.mode))
    }

    fn from_file(&self, path: &str) -> Result<Target> {
        let blob = Blob::new(self.repo.workspace.read_file(Path::new(path))?);
        let oid = self.repo.database.hash_object(&blob);
        let mode = Entry::mode_for_stat(&self.repo.stats[path]);

        Ok(Target::new(path.to_string(), oid, Some(mode)))
    }

    fn from_nothing(&self, path: &str) -> Target {
        Target::new(path.to_string(), NULL_OID.to_string(), None)
    }

    fn short(&self, oid: &str) -> String {
        self.repo.database.short_oid(oid)
    }

    fn print_diff(&self, a: &mut Target, b: &mut Target) {
        if a.oid == b.oid && a.mode == b.mode {
            return;
        }

        a.path = format!("a/{}", a.path);
        b.path = format!("b/{}", b.path);

        println!("diff --git {} {}", a.path, b.path);
        self.print_diff_mode(&a, &b);
        self.print_diff_content(&a, &b);
    }

    fn print_diff_mode(&self, a: &Target, b: &Target) {
        if a.mode.is_none() {
            println!("new file mode {:o}", b.mode.unwrap());
        } else if b.mode.is_none() {
            println!("deleted file mode {:o}", a.mode.unwrap());
        } else if a.mode != b.mode {
            println!("old mode {:o}", a.mode.unwrap());
            println!("new mode {:o}", b.mode.unwrap());
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
    }
}

struct Target {
    path: String,
    oid: String,
    mode: Option<u32>,
}

impl Target {
    fn new(path: String, oid: String, mode: Option<u32>) -> Self {
        Target { path, oid, mode }
    }

    fn diff_path(&self) -> &str {
        match self.mode {
            Some(_) => &self.path,
            None => NULL_PATH,
        }
    }
}
