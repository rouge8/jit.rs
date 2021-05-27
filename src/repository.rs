use crate::database::Database;
use crate::index::Index;
use crate::refs::Refs;
use crate::workspace::Workspace;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Repository {
    git_path: PathBuf,
    pub database: Database,
    pub index: Index,
    pub refs: Refs,
    pub workspace: Workspace,
}

impl Repository {
    pub fn new(git_path: PathBuf) -> Self {
        Repository {
            git_path: git_path.clone(),
            database: Database::new(git_path.join("objects")),
            index: Index::new(git_path.join("index")),
            refs: Refs::new(git_path.clone()),
            workspace: Workspace::new(git_path.parent().unwrap().to_path_buf()),
        }
    }
}
