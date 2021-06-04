use crate::commands::CommandContext;
use crate::database::blob::Blob;
use crate::errors::Result;
use crate::index::Entry;
use crate::repository::Repository;
use crate::util::path_to_string;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};

pub struct Status {
    root_dir: PathBuf,
    repo: Repository,
    stats: HashMap<String, fs::Metadata>,
    changes: HashMap<String, HashSet<ChangeType>>,
    changed: BTreeSet<String>,
    untracked: BTreeSet<String>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum ChangeType {
    WorkspaceDeleted,
    WorkspaceModified,
}

impl Status {
    pub fn new(ctx: CommandContext) -> Self {
        Self {
            root_dir: ctx.dir,
            repo: ctx.repo,
            stats: HashMap::new(),
            changes: HashMap::new(),
            changed: BTreeSet::new(),
            untracked: BTreeSet::new(),
        }
    }

    pub fn run(&mut self) -> Result<()> {
        self.repo.index.load_for_update()?;

        self.scan_workspace(&self.root_dir.clone())?;
        self.detect_workspace_changes()?;

        self.repo.index.write_updates()?;

        self.print_results();

        Ok(())
    }

    fn print_results(&self) {
        for path in &self.changed {
            let status = self.status_for(&path);
            println!("{} {}", status, path);
        }
        for path in &self.untracked {
            println!("?? {}", path);
        }
    }

    fn status_for(&self, path: &str) -> &str {
        let changes = &self.changes[path];

        if changes.contains(&ChangeType::WorkspaceModified) {
            " M"
        } else if changes.contains(&ChangeType::WorkspaceDeleted) {
            " D"
        } else {
            "  "
        }
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
        // We have to iterate over `cloned_entries` rather than `self.repo.index.entries` because
        // Rust will not let us borrow self as mutable more than one time: first with
        // `self.repo.index.entries.values_mut()` and second with `self.check_index_entry()`.
        let mut cloned_entries = self.repo.index.entries.clone();
        for mut entry in cloned_entries.values_mut() {
            self.check_index_entry(&mut entry)?;
        }

        // Update `self.repo.index.entries` with the entries that were modified in
        // `self.check_index_entry()`
        for (key, val) in cloned_entries {
            self.repo.index.entries.insert(key, val);
        }

        Ok(())
    }

    fn record_change(&mut self, path: &str, r#type: ChangeType) {
        self.changed.insert(path.to_string());
        self.changes
            .entry(path.to_string())
            .or_insert_with(HashSet::new)
            .insert(r#type);
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

    fn check_index_entry(&mut self, entry: &mut Entry) -> Result<()> {
        let stat = match self.stats.get(&entry.path) {
            Some(stat) => stat,
            None => {
                self.record_change(&entry.path, ChangeType::WorkspaceDeleted);
                return Ok(());
            }
        };

        if !entry.stat_match(&stat) {
            self.record_change(&entry.path, ChangeType::WorkspaceModified);
            return Ok(());
        }

        if entry.times_match(&stat) {
            return Ok(());
        }

        let data = self.repo.workspace.read_file(&PathBuf::from(&entry.path))?;
        let blob = Blob::new(data);
        let oid = self.repo.database.hash_object(&blob);

        if entry.oid == oid {
            self.repo.index.update_entry_stat(entry, &stat);
        } else {
            self.record_change(&entry.path, ChangeType::WorkspaceModified);
        }

        Ok(())
    }
}
