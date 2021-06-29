use crate::database::tree::TreeEntry;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Trie {
    matched: bool,
    children: HashMap<PathBuf, Trie>,
}

impl Trie {
    pub fn new(matched: bool) -> Self {
        Self {
            matched,
            children: HashMap::new(),
        }
    }

    pub fn from_paths(paths: &[PathBuf]) -> Self {
        let mut root = Trie::node();

        if paths.is_empty() {
            root.matched = true;
        }

        for path in paths {
            let names: Vec<_> = path.iter().map(PathBuf::from).collect();

            let mut trie = root
                .children
                .entry(names[0].clone())
                .or_insert_with(Trie::node);

            for name in &names[1..] {
                trie = trie
                    .children
                    .entry(name.to_owned())
                    .or_insert_with(Trie::node);
            }

            trie.matched = true;
        }

        root
    }

    fn node() -> Self {
        Trie {
            matched: false,
            children: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PathFilter {
    routes: Trie,
    pub path: PathBuf,
}

impl PathFilter {
    pub fn build(paths: &[PathBuf]) -> Self {
        Self::new(Some(Trie::from_paths(paths)), None)
    }

    pub fn new(routes: Option<Trie>, path: Option<PathBuf>) -> Self {
        let routes = routes.unwrap_or_else(|| Trie::new(true));
        let path = path.unwrap_or_else(PathBuf::new);

        Self { routes, path }
    }

    pub fn each_entry(&self, entries: &BTreeMap<PathBuf, TreeEntry>) -> Vec<(PathBuf, TreeEntry)> {
        let mut result = vec![];

        for (name, entry) in entries {
            if self.routes.matched || self.routes.children.contains_key(name) {
                result.push((name.to_owned(), entry.to_owned()));
            }
        }

        result
    }

    pub fn join(&self, name: PathBuf) -> PathFilter {
        let next_routes = if self.routes.matched {
            self.routes.clone()
        } else {
            self.routes.children[&name].clone()
        };

        PathFilter::new(Some(next_routes), Some(self.path.join(name)))
    }
}
