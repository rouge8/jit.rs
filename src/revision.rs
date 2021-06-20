use crate::database::ParsedObject;
use crate::errors::{Error, Result};
use crate::repository::Repository;
use lazy_static::lazy_static;
use regex::{Regex, RegexSet};
use std::collections::HashMap;

lazy_static! {
    static ref INVALID_NAME: RegexSet = RegexSet::new(&[
        r"^\.",
        r"^/\.",
        r"^\.\.",
        r"^/",
        r"/$",
        r"\.lock$",
        r"@\{",
        r"[\x00-\x20*:?\[\\^~\x7f]",
    ])
    .unwrap();
    static ref PARENT: Regex = Regex::new(r"^(.+)\^$").unwrap();
    static ref ANCESTOR: Regex = Regex::new(r"^(.+)~(\d+)$").unwrap();
    static ref REF_ALIASES: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("@", "HEAD");
        m
    };
}

#[derive(Debug)]
pub struct Revision<'a> {
    repo: &'a mut Repository,
    expr: String,
    query: Option<Rev>,
}

impl<'a> Revision<'a> {
    pub fn new(repo: &'a mut Repository, expr: &str) -> Self {
        Self {
            repo,
            expr: expr.to_string(),
            query: Self::parse(expr),
        }
    }

    pub fn valid_ref(revision: &str) -> bool {
        !INVALID_NAME.is_match(revision)
    }

    pub fn resolve(&mut self) -> Result<String> {
        if self.query.is_some() {
            let query = self.query.as_ref().unwrap().clone();
            let oid = query.resolve(self)?;

            if let Some(oid) = oid {
                return Ok(oid);
            }
        }

        return Err(Error::InvalidObject(format!(
            "Not a valid object name: '{}'.",
            self.expr
        )));
    }

    pub fn read_ref(&self, name: &str) -> Result<Option<String>> {
        self.repo.refs.read_ref(name)
    }

    pub fn commit_parent(&mut self, oid: Option<String>) -> Result<Option<String>> {
        match oid {
            Some(oid) => {
                let commit = self.repo.database.load(&oid)?;
                match commit {
                    ParsedObject::Commit(commit) => Ok(commit.parent.clone()),
                    _ => unreachable!(),
                }
            }
            None => Ok(None),
        }
    }

    fn parse(revision: &str) -> Option<Rev> {
        if let Some(r#match) = PARENT.captures(revision) {
            Revision::parse(&r#match[1]).map(|rev| Rev::Parent { rev: Box::new(rev) })
        } else if let Some(r#match) = ANCESTOR.captures(revision) {
            Revision::parse(&r#match[1]).map(|rev| Rev::Ancestor {
                rev: Box::new(rev),
                n: r#match[2].parse().unwrap(),
            })
        } else if Revision::valid_ref(revision) {
            let name = match REF_ALIASES.get(revision) {
                Some(name) => name,
                None => revision,
            };
            Some(Rev::Ref {
                name: name.to_string(),
            })
        } else {
            None
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum Rev {
    Ref { name: String },
    Parent { rev: Box<Rev> },
    Ancestor { rev: Box<Rev>, n: i32 },
}

impl Rev {
    pub fn resolve(&self, context: &mut Revision) -> Result<Option<String>> {
        match self {
            Rev::Ref { name } => context.read_ref(name),
            Rev::Parent { rev } => {
                let oid = rev.resolve(context)?;
                context.commit_parent(oid)
            }
            Rev::Ancestor { rev, n } => {
                let mut oid = rev.resolve(context)?;
                for _ in 0..*n {
                    oid = context.commit_parent(oid)?;
                }
                Ok(oid)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_parse(revision: &str, rev: Rev) {
        assert_eq!(Revision::parse(revision), Some(rev));
    }

    #[test]
    fn parse_head() {
        assert_parse(
            "HEAD",
            Rev::Ref {
                name: String::from("HEAD"),
            },
        );
    }

    #[test]
    fn parse_at() {
        assert_parse(
            "@",
            Rev::Ref {
                name: String::from("HEAD"),
            },
        );
    }

    #[test]
    fn parse_a_branch_name() {
        assert_parse(
            "main",
            Rev::Ref {
                name: String::from("main"),
            },
        );
    }

    #[test]
    fn parse_an_object_id() {
        assert_parse(
            "8d079a148af9278aa26a2dfa905dd01ab1e9296b",
            Rev::Ref {
                name: String::from("8d079a148af9278aa26a2dfa905dd01ab1e9296b"),
            },
        );
    }

    #[test]
    fn parse_a_parent_ref() {
        assert_parse(
            "@^",
            Rev::Parent {
                rev: Box::new(Rev::Ref {
                    name: String::from("HEAD"),
                }),
            },
        );
    }

    #[test]
    fn parse_a_chain_of_parent_refs() {
        assert_parse(
            "main^^^",
            Rev::Parent {
                rev: Box::new(Rev::Parent {
                    rev: Box::new(Rev::Parent {
                        rev: Box::new(Rev::Ref {
                            name: String::from("main"),
                        }),
                    }),
                }),
            },
        );
    }

    #[test]
    fn parse_an_ancestor_ref() {
        assert_parse(
            "HEAD~42",
            Rev::Ancestor {
                rev: Box::new(Rev::Ref {
                    name: String::from("HEAD"),
                }),
                n: 42,
            },
        );
    }

    #[test]
    fn parse_a_chain_of_parents_and_ancestors() {
        assert_parse(
            "@~2^^~3",
            Rev::Ancestor {
                rev: Box::new(Rev::Parent {
                    rev: Box::new(Rev::Parent {
                        rev: Box::new(Rev::Ancestor {
                            rev: Box::new(Rev::Ref {
                                name: String::from("HEAD"),
                            }),
                            n: 2,
                        }),
                    }),
                }),
                n: 3,
            },
        );
    }
}
