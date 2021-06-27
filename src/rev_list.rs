use crate::database::commit::Commit;
use crate::database::ParsedObject;
use crate::errors::{Error, Result};
use crate::repository::Repository;
use crate::revision::{Revision, COMMIT};

pub struct RevList<'a> {
    repo: &'a Repository,
    current_oid: Option<String>,
}

impl<'a> RevList<'a> {
    pub fn new(repo: &'a Repository, start: String) -> Result<Self> {
        let current_oid = Revision::new(&repo, &start).resolve(Some(COMMIT))?;

        Ok(Self {
            repo,
            current_oid: Some(current_oid),
        })
    }
}

impl<'a> Iterator for RevList<'a> {
    type Item = Result<Commit>;

    fn next(&mut self) -> Option<Self::Item> {
        self.current_oid.as_ref()?;

        match self.repo.database.load(&self.current_oid.as_ref().unwrap()) {
            Ok(ParsedObject::Commit(commit)) => {
                self.current_oid = commit.parent.clone();

                Some(Ok(commit))
            }
            Err(err) => Some(Err(Error::Io(err))),
            _ => unreachable!(),
        }
    }
}
