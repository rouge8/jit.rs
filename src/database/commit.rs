use crate::database::author::Author;
use crate::database::object::Object;
use crate::database::ParsedObject;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Commit {
    pub parent: Option<String>,
    pub tree: String,
    pub author: Author,
    pub message: String,
}

impl Commit {
    pub fn new(parent: Option<String>, tree: String, author: Author, message: String) -> Self {
        Commit {
            parent,
            tree,
            author,
            message,
        }
    }

    pub fn parse(data: &[u8]) -> ParsedObject {
        let mut data = std::str::from_utf8(data).expect("Invalid UTF-8");

        let mut headers: HashMap<&str, &str> = HashMap::new();

        loop {
            let (line, rest) = data.split_once("\n").unwrap();
            data = rest;
            let line = line.trim();

            if line.is_empty() {
                let parent = headers.get("parent").map(|parent| parent.to_string());
                break ParsedObject::Commit(Commit::new(
                    parent,
                    headers["tree"].to_string(),
                    Author::parse(headers["author"]),
                    data.to_string(),
                ));
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
