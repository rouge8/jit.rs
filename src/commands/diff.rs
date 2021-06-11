use crate::commands::CommandContext;
use crate::database::blob::Blob;
use crate::errors::Result;
use crate::index::Entry;
use crate::repository::{ChangeType, Repository};
use crate::util::path_to_string;
use lazy_static::lazy_static;
use std::path::Path;

lazy_static! {
    static ref NULL_OID: String = "0".repeat(40);
}
const NULL_PATH: &str = "/dev/null";

pub struct Diff {
    repo: Repository,
}

impl Diff {
    pub fn new(ctx: CommandContext) -> Self {
        Self { repo: ctx.repo }
    }

    pub fn run(&mut self) -> Result<()> {
        self.repo.index.load()?;
        self.repo.initialize_status()?;

        for (path, state) in &self.repo.workspace_changes {
            match state {
                ChangeType::Modified => self.diff_file_modified(&path)?,
                ChangeType::Deleted => self.diff_file_deleted(&path),
                _ => unreachable!(),
            }
        }

        Ok(())
    }

    fn diff_file_modified(&self, path: &str) -> Result<()> {
        let entry = self.repo.index.entry_for_path(path);
        let a_oid = &entry.oid;
        let a_mode = format!("{:o}", entry.mode);
        let a_path = Path::new("a").join(path);

        let blob = Blob::new(self.repo.workspace.read_file(Path::new(path))?);
        let b_oid = self.repo.database.hash_object(&blob);
        let b_mode = format!("{:o}", Entry::mode_for_stat(&self.repo.stats[path]));
        let b_path = Path::new("b").join(path);

        println!(
            "diff --git {} {}",
            path_to_string(&a_path),
            path_to_string(&b_path)
        );

        if a_mode != b_mode {
            println!("old mode {}", a_mode);
            println!("new mode {}", b_mode);
        }

        if a_oid == &b_oid {
            return Ok(());
        }

        let mut oid_range = format!("index {}..{}", self.short(a_oid), self.short(&b_oid));
        if a_mode == b_mode {
            oid_range.push(' ');
            oid_range.push_str(&a_mode);
        }

        println!("{}", oid_range);
        println!("--- {}", path_to_string(&a_path));
        println!("+++ {}", path_to_string(&b_path));

        Ok(())
    }

    fn diff_file_deleted(&self, path: &str) {
        let entry = self.repo.index.entry_for_path(path);
        let a_oid = &entry.oid;
        let a_mode = format!("{:o}", entry.mode);
        let a_path = Path::new("a").join(path);

        let b_oid = &NULL_OID;
        let b_path = Path::new("b").join(path);

        println!(
            "diff --git {} {}",
            path_to_string(&a_path),
            path_to_string(&b_path)
        );
        println!("deleted file mode {}", a_mode);
        println!("index {}..{}", self.short(a_oid), self.short(&b_oid));
        println!("--- {}", path_to_string(&a_path));
        println!("+++ {}", NULL_PATH);
    }

    fn short(&self, oid: &str) -> String {
        self.repo.database.short_oid(oid)
    }
}
