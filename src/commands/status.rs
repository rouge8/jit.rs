use crate::commands::CommandContext;
use crate::database::blob::Blob;
use crate::errors::Result;
use crate::index::Entry;
use crate::repository::Repository;
use crate::util::path_to_string;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};

pub struct Status {
    root_dir: PathBuf,
    repo: Repository,
    stats: HashMap<String, fs::Metadata>,
    changed: BTreeSet<String>,
    untracked: BTreeSet<String>,
}

impl Status {
    pub fn new(ctx: CommandContext) -> Self {
        Self {
            root_dir: ctx.dir,
            repo: ctx.repo,
            stats: HashMap::new(),
            changed: BTreeSet::new(),
            untracked: BTreeSet::new(),
        }
    }

    pub fn run(&mut self) -> Result<()> {
        self.repo.index.load()?;

        self.scan_workspace(&self.root_dir.clone())?;
        self.detect_workspace_changes()?;

        for path in &self.changed {
            println!(" M {}", path);
        }
        for path in &self.untracked {
            println!("?? {}", path);
        }

        Ok(())
    }

    fn scan_workspace(&mut self, prefix: &Path) -> Result<()> {
        for (path, stat) in &self.repo.workspace.list_dir(prefix)? {
            if self.repo.index.tracked(path) {
                if stat.is_file() {
                    self.stats.insert(path_to_string(path), stat.clone());
                } else if stat.is_dir() {
                    self.scan_workspace(&path)?;
                }
            } else if self.trackable_file(&path, &stat)? {
                let mut path = path_to_string(path);
                if stat.is_dir() {
                    path.push(MAIN_SEPARATOR);
                }
                self.untracked.insert(path);
            }
        }

        Ok(())
    }

    fn detect_workspace_changes(&mut self) -> Result<()> {
        for entry in self.repo.index.entries.values() {
            if self.index_entry_changed(&entry)? {
                self.changed.insert(entry.path.clone());
            }
        }

        Ok(())
    }

    fn trackable_file(&mut self, path: &Path, stat: &fs::Metadata) -> Result<bool> {
        if stat.is_file() {
            return Ok(!self.repo.index.tracked(path));
        } else if !stat.is_dir() {
            return Ok(false);
        }

        let items = self.repo.workspace.list_dir(path)?;
        let files = items.iter().filter(|(_, item_stat)| item_stat.is_file());
        let dirs = items.iter().filter(|(_, item_stat)| item_stat.is_dir());

        for (item_path, item_stat) in files.chain(dirs) {
            if self.trackable_file(item_path, item_stat)? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn index_entry_changed(&self, entry: &Entry) -> Result<bool> {
        let stat = &self.stats[&entry.path];
        if !entry.stat_match(&stat) {
            return Ok(true);
        }

        let data = self.repo.workspace.read_file(&PathBuf::from(&entry.path))?;
        let blob = Blob::new(data);
        let oid = self.repo.database.hash_object(&blob);

        Ok(entry.oid != oid)
    }
}
