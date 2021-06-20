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
struct Revision;

#[derive(Debug, PartialEq, Eq)]
enum Rev {
    Ref { name: String },
    Parent { rev: Box<Rev> },
    Ancestor { rev: Box<Rev>, n: i32 },
}

impl Revision {
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

    fn valid_ref(revision: &str) -> bool {
        !INVALID_NAME.is_match(revision)
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
