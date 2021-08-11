use crate::commands::CommandContext;
use crate::database::author::Author;
use crate::database::commit::Commit;
use crate::database::object::Object;
use crate::database::tree::Tree;
use crate::database::Database;
use crate::errors::{Error, Result};
use crate::refs::HEAD;
use crate::repository::pending_commit::PendingCommit;
use chrono::{DateTime, Local};
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

pub const CONFLICT_MESSAGE: &str = "\
hint: Fix them up in the work tree, and then use 'jit add/rm <file>'
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

    pub fn read_message(&self, message: Option<&str>, file: Option<&Path>) -> Result<String> {
        let message = if let Some(message) = message {
            format!("{}\n", message)
        } else if let Some(file) = file {
            read_to_string(file)?
        } else {
            String::new()
        };

        Ok(message)
    }

    pub fn write_commit(&self, parents: Vec<String>, message: Option<&str>) -> Result<Commit> {
        let message = if let Some(message) = message {
            message
        } else {
            ""
        };
        if message.is_empty() {
            let mut stderr = self.ctx.stderr.borrow_mut();
            writeln!(stderr, "Aborting commit due to empty commit message.")?;
            return Err(Error::Exit(1));
        }

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

    pub fn print_commit(&self, commit: &Commit) -> Result<()> {
        let r#ref = self.ctx.repo.refs.current_ref(HEAD)?;
        let mut info = if r#ref.is_head() {
            String::from("detached HEAD")
        } else {
            self.ctx.repo.refs.short_name(&r#ref)
        };
        let oid = Database::short_oid(&commit.oid());

        if commit.parent().is_none() {
            info.push_str(" (root-commit)");
        }
        info.push_str(&format!(" {}", oid));

        let mut stdout = self.ctx.stdout.borrow_mut();
        writeln!(stdout, "[{}] {}", info, commit.title_line(),)?;

        Ok(())
    }

    pub fn resume_merge(&self) -> Result<()> {
        self.handle_conflicted_index()?;

        let parents = vec![
            self.ctx.repo.refs.read_head()?.unwrap(),
            self.pending_commit.merge_oid()?,
        ];
        self.write_commit(parents, Some(&self.pending_commit.merge_message()?))?;

        self.pending_commit.clear()?;
        Err(Error::Exit(0))
    }

    pub fn commit_message_path(&self) -> PathBuf {
        self.ctx.repo.git_path.join("COMMIT_EDITMSG")
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
