use crate::database::author::Author;
use crate::database::object::Object;
use crate::database::ParsedObject;
use sha1::{Digest, Sha1};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Commit {
    pub parent: Option<String>,
    pub tree: String,
    pub author: Author,
    pub message: String,
    oid: Option<String>,
}

impl Commit {
    pub fn new(parent: Option<String>, tree: String, author: Author, message: String) -> Self {
        Commit {
            parent,
            tree,
            author,
            message,
            oid: None,
        }
    }

    pub fn parse(data: &[u8], oid: &str) -> ParsedObject {
        let mut data = std::str::from_utf8(data).expect("Invalid UTF-8");

        let mut headers: HashMap<&str, &str> = HashMap::new();

        loop {
            let (line, rest) = data.split_once("\n").unwrap();
            data = rest;
            let line = line.trim();

            if line.is_empty() {
                let parent = headers.get("parent").map(|parent| parent.to_string());
                break ParsedObject::Commit(Commit {
                    parent,
                    tree: headers["tree"].to_string(),
                    author: Author::parse(headers["author"]),
                    message: data.to_string(),
                    oid: Some(oid.to_string()),
                });
            }

            let (key, value) = line.split_once(" ").unwrap();
            headers.insert(key, value);
        }
    }

    pub fn title_line(&self) -> String {
        self.message.lines().next().unwrap().to_string()
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
        let parent = &self.parent;

        let mut lines = vec![format!("tree {}", &self.tree)];
        if let Some(parent) = parent {
            lines.push(format!("parent {}", parent));
        }
        lines.append(&mut vec![
            format!("author {}", &self.author),
            format!("committer {}", &self.author),
            "".to_string(),
            self.message.clone(),
        ]);

        lines.join("\n").into_bytes()
    }
}
