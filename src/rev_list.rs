use crate::database::commit::Commit;
use crate::database::object::Object;
use crate::database::tree_diff::{Differ, TreeDiffChanges};
use crate::errors::Result;
use crate::path_filter::PathFilter;
use crate::repository::Repository;
use crate::revision::{Revision, COMMIT, HEAD};
use lazy_static::lazy_static;
use regex::Regex;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

lazy_static! {
    static ref RANGE: Regex = Regex::new(r"^(.*)\.\.(.*)$").unwrap();
    static ref EXCLUDE: Regex = Regex::new(r"^\^(.+)$").unwrap();
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Flag {
    Added,
    Seen,
    Uninteresting,
    Treesame,
}

#[derive(Debug, Clone)]
pub struct RevList<'a> {
    repo: &'a Repository,
    commits: HashMap<String, Commit>,
    flags: RefCell<HashMap<String, HashSet<Flag>>>,
    queue: VecDeque<Commit>,
    limited: bool,
    prune: Vec<PathBuf>,
    diffs: RefCell<HashMap<(Option<String>, String), TreeDiffChanges>>,
    output: VecDeque<Commit>,
    filter: PathFilter,
}

impl<'a> RevList<'a> {
    pub fn new(repo: &'a Repository, revs: &[String]) -> Result<Self> {
        let mut rev_list = Self {
            repo,
            commits: HashMap::new(),
            flags: RefCell::new(HashMap::new()),
            queue: VecDeque::new(),
            limited: false,
            prune: Vec::new(),
            diffs: RefCell::new(HashMap::new()),
            output: VecDeque::new(),
            // A temporary `PathFilter` that will be replaced later in this function
            filter: PathFilter::new(None, None),
        };

        for rev in revs {
            rev_list.handle_revision(&rev)?;
        }
        if rev_list.queue.is_empty() {
            rev_list.handle_revision(HEAD)?;
        }

        rev_list.filter = PathFilter::build(&rev_list.prune);

        Ok(rev_list)
    }

    fn handle_revision(&mut self, rev: &str) -> Result<()> {
        if self.repo.workspace.stat_file(&PathBuf::from(rev)).is_ok() {
            self.prune.push(PathBuf::from(rev));
        } else if let Some(r#match) = RANGE.captures(&rev) {
            self.set_start_point(&r#match[1], false)?;
            self.set_start_point(&r#match[2], true)?;
        } else if let Some(r#match) = EXCLUDE.captures(&rev) {
            self.set_start_point(&r#match[1], false)?;
        } else {
            self.set_start_point(&rev, true)?;
        }

        Ok(())
    }

    fn set_start_point(&mut self, rev: &str, interesting: bool) -> Result<()> {
        let rev = if rev.is_empty() { HEAD } else { rev };

        let oid = Revision::new(&self.repo, &rev).resolve(Some(COMMIT))?;

        let commit = self.load_commit(Some(&oid))?;
        self.enqueue_commit(commit.as_ref());

        if !interesting {
            self.limited = true;
            self.mark(&oid, Flag::Uninteresting);
            self.mark_parents_uninteresting(commit.as_ref());
        }

        Ok(())
    }

    fn enqueue_commit(&mut self, commit: Option<&Commit>) {
        if commit.is_none() {
            return;
        }
        let commit = commit.unwrap();

        // We're seeing this commit for the first time
        if !self.mark(&commit.oid(), Flag::Seen) {
            let index = self.queue.iter().position(|c| c.date() < commit.date());

            if let Some(index) = index {
                self.queue.insert(index, commit.to_owned());
            } else {
                self.queue.push_back(commit.to_owned());
            }
        }
    }

    fn limit_list(&mut self) -> Result<()> {
        while self.still_interesting() {
            let commit = self.queue.pop_front();
            if let Some(commit) = commit {
                self.add_parents(&commit)?;

                if !self.is_marked(&commit.oid(), Flag::Uninteresting) {
                    self.output.push_back(commit);
                }
            }
        }

        self.queue.clear();
        self.queue.append(&mut self.output);

        Ok(())
    }

    fn still_interesting(&self) -> bool {
        if self.queue.is_empty() {
            return false;
        }

        let oldest_out = self.output.back();
        let newest_in = self.queue.front().unwrap();

        if oldest_out.is_some() && oldest_out.unwrap().date() <= newest_in.date() {
            return true;
        }

        if self
            .queue
            .iter()
            .any(|commit| !self.is_marked(&commit.oid(), Flag::Uninteresting))
        {
            return true;
        }

        false
    }

    fn add_parents(&mut self, commit: &Commit) -> Result<()> {
        if self.mark(&commit.oid(), Flag::Added) {
            return Ok(());
        }

        let parents: Vec<_> = commit
            .parents
            .iter()
            .map(|oid| self.load_commit(Some(&oid)).unwrap())
            .collect();

        if self.is_marked(&commit.oid(), Flag::Uninteresting) {
            for parent in &parents {
                self.mark_parents_uninteresting(parent.as_ref());
            }
        } else {
            self.simplify_commit(&commit)?;
        }

        for parent in &parents {
            self.enqueue_commit(parent.as_ref());
        }

        Ok(())
    }

    fn mark_parents_uninteresting(&mut self, commit: Option<&Commit>) {
        if commit.is_none() {
            return;
        }

        let mut queue: VecDeque<_> = commit.unwrap().parents.iter().cloned().collect();

        while !queue.is_empty() {
            let oid = queue.pop_front().unwrap();
            if !self.mark(&oid, Flag::Uninteresting) {
                continue;
            }
            let commit = self.commits.get(&oid);

            if let Some(commit) = commit {
                for parent in &commit.parents {
                    queue.push_back(parent.to_owned());
                }
            }
        }
    }

    fn load_commit(&mut self, oid: Option<&str>) -> Result<Option<Commit>> {
        if oid.is_none() {
            return Ok(None);
        }
        let oid = oid.unwrap();
        if !self.commits.contains_key(oid) {
            let commit = self.repo.database.load_commit(&oid)?;
            self.commits.insert(oid.to_string(), commit);
        }

        Ok(Some(self.commits[oid].to_owned()))
    }

    fn mark(&self, oid: &str, flag: Flag) -> bool {
        let mut all_flags = self.flags.borrow_mut();
        let flags = all_flags
            .entry(oid.to_string())
            .or_insert_with(HashSet::new);

        if flags.contains(&flag) {
            true
        } else {
            flags.insert(flag);
            false
        }
    }

    fn is_marked(&self, oid: &str, flag: Flag) -> bool {
        let flags = self.flags.borrow();
        if flags.contains_key(oid) {
            flags[oid].contains(&flag)
        } else {
            false
        }
    }

    fn simplify_commit(&self, commit: &Commit) -> Result<()> {
        if self.prune.is_empty() {
            return Ok(());
        }

        if self
            .tree_diff(commit.parent().as_deref(), Some(&commit.oid()), None)?
            .is_empty()
        {
            self.mark(&commit.oid(), Flag::Treesame);
        }

        Ok(())
    }
}

impl<'a> Differ for RevList<'a> {
    fn tree_diff(
        &self,
        old_oid: Option<&str>,
        new_oid: Option<&str>,
        _filter: Option<&PathFilter>,
    ) -> Result<TreeDiffChanges> {
        let key = (old_oid.map(|s| s.to_owned()), new_oid.unwrap().to_string());

        let mut diffs = self.diffs.borrow_mut();

        Ok(diffs
            .entry(key)
            .or_insert_with(|| {
                self.repo
                    .database
                    .tree_diff(old_oid.as_deref(), new_oid, Some(&self.filter))
                    .unwrap()
            })
            .to_owned())
    }
}

impl<'a> Iterator for RevList<'a> {
    type Item = Commit;

    fn next(&mut self) -> Option<Self::Item> {
        if self.limited {
            self.limit_list().unwrap();
        }

        if let Some(commit) = self.queue.pop_front() {
            if !self.limited {
                self.add_parents(&commit).unwrap();
            }

            if self.is_marked(&commit.oid(), Flag::Uninteresting)
                || self.is_marked(&commit.oid(), Flag::Treesame)
            {
                self.next()
            } else {
                Some(commit)
            }
        } else {
            None
        }
    }
}
