use crate::commands::CommandContext;
use crate::database::author::Author;
use crate::database::commit::Commit;
use crate::database::entry::Entry;
use crate::database::object::Object;
use crate::database::tree::Tree;
use crate::errors::Result;
use chrono::{DateTime, Local};

pub struct CommitWriter<'a> {
    ctx: &'a CommandContext<'a>,
}

impl<'a> CommitWriter<'a> {
    pub fn new(ctx: &'a CommandContext) -> Self {
        Self { ctx }
    }

    pub fn write_commit(&self, parents: Vec<String>, message: String) -> Result<Commit> {
        let tree = self.write_tree();
        let name = &self.ctx.env["GIT_AUTHOR_NAME"];
        let email = &self.ctx.env["GIT_AUTHOR_EMAIL"];

        let author_date = if let Some(author_date_str) = self.ctx.env.get("GIT_AUTHOR_DATE") {
            DateTime::parse_from_rfc2822(author_date_str).expect("could not parse GIT_AUTHOR_DATE")
        } else {
            let now = Local::now();
            now.with_timezone(now.offset())
        };
        let author = Author::new(name.clone(), email.clone(), author_date);

        let commit = Commit::new(parents, tree.oid(), author, message);
        self.ctx.repo.database.store(&commit)?;
        self.ctx.repo.refs.update_head(&commit.oid())?;

        Ok(commit)
    }

    fn write_tree(&self) -> Tree {
        let entries = self
            .ctx
            .repo
            .index
            .entries
            .values()
            .map(Entry::from)
            .collect();
        let root = Tree::build(entries);
        root.traverse(&|tree| {
            self.ctx.repo.database.store(tree).unwrap();
        });

        root
    }
}
