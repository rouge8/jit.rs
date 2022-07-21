use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::{Regex, RegexSet};

use crate::database::{Database, ParsedObject};
use crate::errors::{Error, Result};
use crate::repository::Repository;

static INVALID_NAME: Lazy<RegexSet> = Lazy::new(|| {
    RegexSet::new(&[
        r"^\.",
        r"^/\.",
        r"^\.\.",
        r"^/",
        r"/$",
        r"\.lock$",
        r"@\{",
        r"[\x00-\x20*:?\[\\^~\x7f]",
    ])
    .unwrap()
});
static PARENT: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(.+)\^(\d*)$").unwrap());
static ANCESTOR: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(.+)~(\d+)$").unwrap());
static REF_ALIASES: Lazy<HashMap<&'static str, &'static str>> =
    Lazy::new(|| HashMap::from([("@", HEAD)]));

pub const COMMIT: &str = "commit";
pub const HEAD: &str = "HEAD";

#[derive(Debug)]
pub struct Revision<'a> {
    repo: &'a Repository,
    expr: String,
    query: Option<Rev>,
    pub errors: Vec<HintedError>,
}

impl<'a> Revision<'a> {
    pub fn new(repo: &'a Repository, expr: &str) -> Self {
        Self {
            repo,
            expr: expr.to_string(),
            query: Self::parse(expr),
            errors: vec![],
        }
    }

    pub fn valid_ref(revision: &str) -> bool {
        !INVALID_NAME.is_match(revision)
    }

    pub fn resolve(&mut self, r#type: Option<&str>) -> Result<String> {
        if self.query.is_some() {
            let query = self.query.as_ref().unwrap().clone();
            let mut oid = query.resolve(self)?;

            if r#type.is_some()
                && self
                    .load_typed_object(oid.as_ref(), r#type.unwrap())?
                    .is_none()
            {
                oid = None;
            }

            if let Some(oid) = oid {
                return Ok(oid);
            }
        }

        return Err(Error::InvalidObject(format!(
            "Not a valid object name: '{}'.",
            self.expr
        )));
    }

    pub fn read_ref(&mut self, name: &str) -> Result<Option<String>> {
        let oid = self.repo.refs.read_ref(name)?;
        if oid.is_some() {
            return Ok(oid);
        }

        let candidates = self.repo.database.prefix_match(name)?;
        if candidates.len() == 1 {
            return Ok(Some(candidates[0].to_string()));
        }

        if candidates.len() > 1 {
            self.log_ambiguous_sha1(name, candidates)?;
        }

        Ok(None)
    }

    pub fn commit_parent(&mut self, oid: Option<String>, n: usize) -> Result<Option<String>> {
        match oid {
            Some(oid) => {
                let commit = self.load_typed_object(Some(&oid), COMMIT)?;
                match commit {
                    Some(ParsedObject::Commit(commit)) => {
                        if commit.parents.is_empty() {
                            Ok(None)
                        } else {
                            Ok(Some(commit.parents[n - 1].clone()))
                        }
                    }
                    _ => Ok(None),
                }
            }
            None => Ok(None),
        }
    }

    fn parse(revision: &str) -> Option<Rev> {
        if let Some(r#match) = PARENT.captures(revision) {
            Revision::parse(&r#match[1]).map(|rev| Rev::Parent {
                rev: Box::new(rev),
                n: r#match[2].parse().unwrap_or(1),
            })
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

    fn load_typed_object(
        &mut self,
        oid: Option<&String>,
        r#type: &str,
    ) -> Result<Option<ParsedObject>> {
        if let Some(oid) = oid {
            let object = self.repo.database.load(oid)?;

            if object.r#type() == r#type {
                Ok(Some(object))
            } else {
                let message = format!("object {} is a {}, not a {}", oid, object.r#type(), r#type);
                self.errors.push(HintedError::new(message, vec![]));
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    fn log_ambiguous_sha1(&mut self, name: &str, mut candidates: Vec<String>) -> Result<()> {
        let message = format!("short SHA1 {} is ambiguous", name);
        let mut hint = vec![String::from("The candidates are:")];

        candidates.sort();
        for oid in candidates {
            let object = self.repo.database.load(&oid)?;
            let short = Database::short_oid(&object.oid());
            let info = format!("  {} {}", short, object.r#type());

            hint.push(if let ParsedObject::Commit(commit) = object {
                format!(
                    "{} {} - {}",
                    info,
                    commit.author.short_date(),
                    commit.title_line(),
                )
            } else {
                info
            });
        }

        self.errors.push(HintedError::new(message, hint));

        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum Rev {
    Ref { name: String },
    Parent { rev: Box<Rev>, n: usize },
    Ancestor { rev: Box<Rev>, n: i32 },
}

impl Rev {
    pub fn resolve(&self, context: &mut Revision) -> Result<Option<String>> {
        match self {
            Rev::Ref { name } => context.read_ref(name),
            Rev::Parent { rev, n } => {
                let oid = rev.resolve(context)?;
                context.commit_parent(oid, *n)
            }
            Rev::Ancestor { rev, n } => {
                let mut oid = rev.resolve(context)?;
                for _ in 0..*n {
                    oid = context.commit_parent(oid, 1)?;
                }
                Ok(oid)
            }
        }
    }
}

#[derive(Debug)]
pub struct HintedError {
    pub message: String,
    pub hint: Vec<String>,
}

impl HintedError {
    pub fn new(message: String, hint: Vec<String>) -> Self {
        Self { message, hint }
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
                n: 1,
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
                        n: 1,
                    }),
                    n: 1,
                }),
                n: 1,
            },
        );
    }

    #[test]
    fn parse_a_parent_ref_with_a_number() {
        assert_parse(
            "@^2",
            Rev::Parent {
                rev: Box::new(Rev::Ref {
                    name: String::from("HEAD"),
                }),
                n: 2,
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
                        n: 1,
                    }),
                    n: 1,
                }),
                n: 3,
            },
        );
    }
}
