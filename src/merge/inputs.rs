use crate::errors::Result;
use crate::merge::bases::Bases;
use crate::repository::Repository;
use crate::revision::{Revision, COMMIT};

pub trait MergeInputs {
    fn left_name(&self) -> String;

    fn right_name(&self) -> String;

    fn left_oid(&self) -> String;

    fn right_oid(&self) -> String;

    fn base_oids(&self) -> Vec<String>;
}

#[derive(Debug)]
pub struct Inputs {
    pub left_name: String,
    pub right_name: String,
    pub left_oid: String,
    pub right_oid: String,
    pub base_oids: Vec<String>,
}

impl Inputs {
    pub fn new(repo: &Repository, left_name: String, right_name: String) -> Result<Self> {
        let left_oid = Self::resolve_rev(repo, &left_name)?;
        let right_oid = Self::resolve_rev(repo, &right_name)?;

        let mut common = Bases::new(&repo.database, &left_oid, &right_oid)?;
        let base_oids = common.find()?;

        Ok(Self {
            left_name,
            right_name,
            left_oid,
            right_oid,
            base_oids,
        })
    }

    pub fn already_merged(&self) -> bool {
        self.base_oids == vec![self.right_oid.clone()]
    }

    pub fn is_fast_forward(&self) -> bool {
        self.base_oids == vec![self.left_oid.clone()]
    }

    fn resolve_rev(repo: &Repository, rev: &str) -> Result<String> {
        Revision::new(repo, rev).resolve(Some(COMMIT))
    }
}

impl MergeInputs for Inputs {
    fn left_name(&self) -> String {
        self.left_name.clone()
    }

    fn right_name(&self) -> String {
        self.right_name.clone()
    }

    fn left_oid(&self) -> String {
        self.left_oid.clone()
    }

    fn right_oid(&self) -> String {
        self.right_oid.clone()
    }

    fn base_oids(&self) -> Vec<String> {
        self.base_oids.clone()
    }
}

#[derive(Debug)]
pub struct CherryPick {
    pub left_name: String,
    pub right_name: String,
    pub left_oid: String,
    pub right_oid: String,
    pub base_oids: Vec<String>,
}

impl CherryPick {
    pub fn new(
        left_name: String,
        right_name: String,
        left_oid: String,
        right_oid: String,
        base_oids: Vec<String>,
    ) -> Self {
        Self {
            left_name,
            right_name,
            left_oid,
            right_oid,
            base_oids,
        }
    }
}

impl MergeInputs for CherryPick {
    fn left_name(&self) -> String {
        self.left_name.clone()
    }

    fn right_name(&self) -> String {
        self.right_name.clone()
    }

    fn left_oid(&self) -> String {
        self.left_oid.clone()
    }

    fn right_oid(&self) -> String {
        self.right_oid.clone()
    }

    fn base_oids(&self) -> Vec<String> {
        self.base_oids.clone()
    }
}
