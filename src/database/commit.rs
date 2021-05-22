use crate::database::author::Author;
use crate::database::object::Object;

#[derive(Debug)]
pub struct Commit {
    pub parent: Option<String>,
    tree: String,
    author: Author,
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
