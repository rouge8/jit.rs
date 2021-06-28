use crate::database::commit::Commit;
use crate::database::object::Object;
use crate::database::ParsedObject;
use crate::errors::Result;
use crate::repository::Repository;
use crate::revision::{Revision, COMMIT, HEAD};
use lazy_static::lazy_static;
use regex::Regex;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};

lazy_static! {
    static ref RANGE: Regex = Regex::new(r"^(.*)\.\.(.*)$").unwrap();
    static ref EXCLUDE: Regex = Regex::new(r"^\^(.+)$").unwrap();
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum Flag {
    Added,
    Seen,
    Uninteresting,
}

#[derive(Debug)]
pub struct RevList<'a> {
    repo: &'a Repository,
    commits: HashMap<String, Commit>,
    flags: RefCell<HashMap<String, HashSet<Flag>>>,
    queue: VecDeque<Commit>,
    limited: bool,
    output: VecDeque<Commit>,
}

impl<'a> RevList<'a> {
    pub fn new(repo: &'a Repository, revs: &[String]) -> Result<Self> {
        let mut rev_list = Self {
            repo,
            commits: HashMap::new(),
            flags: RefCell::new(HashMap::new()),
            queue: VecDeque::new(),
            limited: false,
            output: VecDeque::new(),
        };

        for rev in revs {
            rev_list.handle_revision(&rev)?;
        }
        if rev_list.queue.is_empty() {
            rev_list.handle_revision(HEAD)?;
        }

        Ok(rev_list)
    }

    fn handle_revision(&mut self, rev: &str) -> Result<()> {
        if let Some(r#match) = RANGE.captures(&rev) {
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
        if !self.mark(&commit.oid(), Flag::Added) {
            if let Some(parent) = self.load_commit(commit.parent.as_deref())? {
                if self.is_marked(&commit.oid(), Flag::Uninteresting) {
                    self.mark_parents_uninteresting(Some(commit));
                }

                self.enqueue_commit(Some(&parent));
            }
        }

        Ok(())
    }

    fn mark_parents_uninteresting(&mut self, commit: Option<&Commit>) {
        let mut commit = commit;

        while commit.is_some() && commit.unwrap().parent.is_some() {
            let parent = commit.unwrap().parent.as_ref().unwrap();
            if !self.mark(parent, Flag::Uninteresting) {
                break;
            }
            commit = self.commits.get(parent);
        }
    }

    fn load_commit(&mut self, oid: Option<&str>) -> Result<Option<Commit>> {
        if oid.is_none() {
            return Ok(None);
        }
        let oid = oid.unwrap();
        if !self.commits.contains_key(oid) {
            let commit = match self.repo.database.load(&oid)? {
                ParsedObject::Commit(commit) => commit,
                _ => unreachable!(),
            };
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

            if self.is_marked(&commit.oid(), Flag::Uninteresting) {
                self.next()
            } else {
                Some(commit)
            }
        } else {
            None
        }
    }
}
