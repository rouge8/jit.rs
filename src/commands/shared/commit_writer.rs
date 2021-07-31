use crate::commands::CommandContext;
use crate::database::author::Author;
use crate::database::commit::Commit;
use crate::database::object::Object;
use crate::database::tree::Tree;
use crate::errors::{Error, Result};
use crate::repository::pending_commit::PendingCommit;
use chrono::{DateTime, Local};

pub const CONFLICT_MESSAGE: &str = "\
hint: Fix them up in the work tree, and then use 'jit add <file>'
hint: as appropriate to mark resolution and make a commit.
fatal: Exiting because of an unresolved conflict.";

pub struct CommitWriter<'a> {
    ctx: &'a CommandContext<'a>,
    pub pending_commit: PendingCommit,
}

impl<'a> CommitWriter<'a> {
    pub fn new(ctx: &'a CommandContext) -> Self {
        let pending_commit = ctx.repo.pending_commit();

        Self {
            ctx,
            pending_commit,
        }
    }

    pub fn write_commit(&self, parents: Vec<String>, message: &str) -> Result<Commit> {
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

        let commit = Commit::new(parents, tree.oid(), author, message.to_string());
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
            .map(|entry| entry.to_owned())
            .collect();
        let root = Tree::build(entries);
        root.traverse(&|tree| {
            self.ctx.repo.database.store(tree).unwrap();
        });

        root
    }

    pub fn resume_merge(&self) -> Result<()> {
        self.handle_conflicted_index()?;

        let parents = vec![
            self.ctx.repo.refs.read_head()?.unwrap(),
            self.pending_commit.merge_oid()?,
        ];
        self.write_commit(parents, &self.pending_commit.merge_message()?)?;

        self.pending_commit.clear()?;
        Err(Error::Exit(0))
    }

    fn handle_conflicted_index(&self) -> Result<()> {
        if !self.ctx.repo.index.has_conflict() {
            return Ok(());
        }

        let mut stderr = self.ctx.stderr.borrow_mut();
        writeln!(
            stderr,
            "error: Committing is not possible because you have unmerged files."
        )?;
        writeln!(stderr, "{}", CONFLICT_MESSAGE)?;

        Err(Error::Exit(128))
    }
}
