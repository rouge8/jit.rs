use crate::database::commit::Commit;
use crate::database::object::Object;
use crate::database::ParsedObject;
use crate::errors::Result;
use crate::repository::Repository;
use crate::revision::{Revision, COMMIT, HEAD};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, PartialEq, Eq, Hash)]
enum Flag {
    Added,
    Seen,
}

#[derive(Debug)]
pub struct RevList<'a> {
    repo: &'a Repository,
    commits: HashMap<String, Commit>,
    flags: HashMap<String, HashSet<Flag>>,
    queue: VecDeque<Commit>,
}

impl<'a> RevList<'a> {
    pub fn new(repo: &'a Repository, revs: &[String]) -> Result<Self> {
        let mut rev_list = Self {
            repo,
            commits: HashMap::new(),
            flags: HashMap::new(),
            queue: VecDeque::new(),
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
        let oid = Revision::new(&self.repo, &rev).resolve(Some(COMMIT))?;

        let commit = self.load_commit(Some(&oid))?;
        self.enqueue_commit(commit);

        Ok(())
    }

    fn enqueue_commit(&mut self, commit: Option<Commit>) {
        if commit.is_none() {
            return;
        }
        let commit = commit.unwrap();

        // We're seeing this commit for the first time
        if !self.mark(&commit.oid(), Flag::Seen) {
            let index = self.queue.iter().position(|c| c.date() < commit.date());

            if let Some(index) = index {
                self.queue.insert(index, commit);
            } else {
                self.queue.push_back(commit);
            }
        }
    }

    fn add_parents(&mut self, commit: &Commit) -> Result<()> {
        if !self.mark(&commit.oid(), Flag::Added) {
            if let Some(parent) = self.load_commit(commit.parent.as_deref())? {
                self.enqueue_commit(Some(parent));
            }
        }

        Ok(())
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

    fn mark(&mut self, oid: &str, flag: Flag) -> bool {
        let flags = self
            .flags
            .entry(oid.to_string())
            .or_insert_with(HashSet::new);

        if flags.contains(&flag) {
            true
        } else {
            flags.insert(flag);
            false
        }
    }

    #[allow(dead_code)]
    fn is_marked(&self, oid: &str, flag: Flag) -> bool {
        if self.flags.contains_key(oid) {
            self.flags[oid].contains(&flag)
        } else {
            false
        }
    }
}

impl<'a> Iterator for RevList<'a> {
    type Item = Commit;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(commit) = self.queue.pop_front() {
            self.add_parents(&commit).unwrap();

            Some(commit)
        } else {
            None
        }
    }
}
