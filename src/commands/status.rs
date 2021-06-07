use crate::commands::CommandContext;
use crate::database::blob::Blob;
use crate::database::tree::TreeEntry;
use crate::database::ParsedObject;
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
    head_tree: HashMap<String, TreeEntry>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum ChangeType {
    WorkspaceDeleted,
    WorkspaceModified,
    IndexAdded,
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
            head_tree: HashMap::new(),
        }
    }

    pub fn run(&mut self) -> Result<()> {
        self.repo.index.load_for_update()?;

        self.scan_workspace(&self.root_dir.clone())?;
        self.load_head_tree()?;
        self.check_index_entries()?;

        self.repo.index.write_updates()?;

        self.print_results();

        Ok(())
    }

    fn load_head_tree(&mut self) -> Result<()> {
        let head_oid = self.repo.refs.read_head()?;

        if let Some(head_oid) = head_oid {
            let commit = match self.repo.database.load(head_oid)? {
                ParsedObject::Commit(commit) => commit,
                _ => unreachable!(),
            };
            let tree_oid = commit.tree.clone();
            self.read_tree(tree_oid, PathBuf::new())?;
        }

        Ok(())
    }

    fn read_tree(&mut self, tree_oid: String, pathname: PathBuf) -> Result<()> {
        let tree = match self.repo.database.load(tree_oid)? {
            ParsedObject::Tree(tree) => tree,
            _ => unreachable!(),
        };

        let entries = tree.entries.clone();
        for (name, entry) in entries {
            let path = pathname.join(name);

            if entry.is_tree() {
                self.read_tree(entry.oid(), path)?;
            } else {
                self.head_tree.insert(path_to_string(&path), entry);
            }
        }

        Ok(())
    }

    fn check_index_entries(&mut self) -> Result<()> {
        // We have to iterate over `cloned_entries` rather than `self.repo.index.entries` because
        // Rust will not let us borrow self as mutable more than one time: first with
        // `self.repo.index.entries.values_mut()` and second with `self.check_index_entry()`.
        let mut cloned_entries = self.repo.index.entries.clone();
        for mut entry in cloned_entries.values_mut() {
            self.check_index_against_workspace(&mut entry)?;
            self.check_index_against_head_tree(&entry);
        }

        // Update `self.repo.index.entries` with the entries that were modified in
        // `self.check_index_entry()`
        for (key, val) in cloned_entries {
            self.repo.index.entries.insert(key, val);
        }

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

    fn status_for(&self, path: &str) -> String {
        let changes = &self.changes[path];

        let left = if changes.contains(&ChangeType::IndexAdded) {
            "A"
        } else {
            " "
        };
        let right = if changes.contains(&ChangeType::WorkspaceModified) {
            "M"
        } else if changes.contains(&ChangeType::WorkspaceDeleted) {
            "D"
        } else {
            " "
        };

        left.to_owned() + right
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

    fn check_index_against_workspace(&mut self, entry: &mut Entry) -> Result<()> {
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

    fn check_index_against_head_tree(&mut self, entry: &Entry) {
        if self.head_tree.get(&entry.path).is_none() {
            self.record_change(&entry.path, ChangeType::IndexAdded);
        }
    }
}
