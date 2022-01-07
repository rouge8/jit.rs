use crate::database::author::Author;
use crate::database::object::Object;
use crate::database::ParsedObject;
use chrono::{DateTime, FixedOffset};
use sha1::{digest::Update, Digest, Sha1};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Commit {
    pub parents: Vec<String>,
    pub tree: String,
    pub author: Author,
    pub committer: Author,
    pub message: String,
    oid: Option<String>,
}

impl Commit {
    pub fn new(
        parents: Vec<String>,
        tree: String,
        author: Author,
        committer: Author,
        message: String,
    ) -> Self {
        Commit {
            parents,
            tree,
            author,
            committer,
            message,
            oid: None,
        }
    }

    pub fn parse(data: &[u8], oid: &str) -> ParsedObject {
        let mut data = std::str::from_utf8(data).expect("Invalid UTF-8");

        let mut headers: HashMap<&str, Vec<&str>> = HashMap::new();

        loop {
            let (line, rest) = data.split_once("\n").unwrap();
            data = rest;
            let line = line.trim();

            if line.is_empty() {
                let parents = headers
                    .entry("parent")
                    .or_insert_with(Vec::new)
                    .iter()
                    .map(|parent| parent.to_string())
                    .collect();
                break ParsedObject::Commit(Commit {
                    parents,
                    tree: headers["tree"][0].to_string(),
                    author: Author::parse(headers["author"][0]),
                    committer: Author::parse(headers["committer"][0]),
                    message: data.to_string(),
                    oid: Some(oid.to_string()),
                });
            }

            let (key, value) = line.split_once(" ").unwrap();
            headers.entry(key).or_insert_with(Vec::new).push(value);
        }
    }

    pub fn title_line(&self) -> String {
        self.message.lines().next().unwrap().to_string()
    }

    pub fn date(&self) -> DateTime<FixedOffset> {
        self.committer.time
    }

    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }

    pub fn parent(&self) -> Option<String> {
        self.parents.first().map(|parent| parent.to_owned())
    }
}

impl Object for Commit {
    fn r#type(&self) -> &str {
        "commit"
    }

    fn oid(&self) -> String {
        match &self.oid {
            Some(oid) => oid.to_string(),
            None => {
                let hash = Sha1::new().chain(&self.content()).finalize();
                format!("{:x}", hash)
            }
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let mut lines = vec![format!("tree {}", &self.tree)];

        for parent in &self.parents {
            lines.push(format!("parent {}", parent));
        }
        lines.append(&mut vec![
            format!("author {}", &self.author),
            format!("committer {}", &self.committer),
            "".to_string(),
            self.message.clone(),
        ]);

        lines.join("\n").into_bytes()
    }
}
