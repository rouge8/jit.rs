use std::env;
use std::path::PathBuf;

use anyhow::Result;
use jit::database::tree::TREE_MODE;
use jit::database::ParsedObject;
use jit::repository::Repository;

fn show_tree(oid: String, prefix: PathBuf) -> Result<()> {
    let repo = repo()?;

    let tree = match repo.database.load(&oid)? {
        ParsedObject::Tree(tree) => tree,
        _ => unreachable!(),
    };

    for (name, entry) in &tree.entries {
        let path = prefix.join(name);

        match entry.mode() {
            TREE_MODE => {
                show_tree(entry.oid(), path)?;
            }
            _ => {
                println!("{:o} {} {:?}", entry.mode(), entry.oid(), path);
            }
        }
    }

    Ok(())
}

fn repo() -> Result<Repository> {
    Ok(Repository::new(env::current_dir()?.join(".git")))
}

fn main() -> Result<()> {
    let repo = repo()?;

    let head_oid = repo.refs.read_head()?.unwrap();
    let commit = repo.database.load_commit(&head_oid)?;

    show_tree(commit.tree, PathBuf::from(""))?;

    Ok(())
}
