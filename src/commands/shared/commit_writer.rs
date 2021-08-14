use crate::commands::commit::COMMIT_NOTES;
use crate::commands::CommandContext;
use crate::database::author::Author;
use crate::database::commit::Commit;
use crate::database::object::Object;
use crate::database::tree::Tree;
use crate::database::Database;
use crate::editor::Editor;
use crate::errors::{Error, Result};
use crate::refs::HEAD;
use crate::repository::pending_commit::{PendingCommit, PendingCommitType};
use chrono::{DateTime, Local};
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

pub const CONFLICT_MESSAGE: &str = "\
hint: Fix them up in the work tree, and then use 'jit add/rm <file>'
hint: as appropriate to mark resolution and make a commit.
fatal: Exiting because of an unresolved conflict.";

const MERGE_NOTES: &str = "\
It looks like you may be committing a merge.
If this is not correct, please remove the file
\t.git/MERGE_HEAD
and try again.\n";

const CHERRY_PICK_NOTES: &str = "\
It looks like you may be committing a cherry-pick.
If this is not correct, please remove the file
\t.git/CHERRY_PICK_HEAD
and try again.\n";

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
        let author = self.current_author();
        let committer = author.clone();
        let commit = Commit::new(parents, tree.oid(), author, committer, message.to_string());

        self.ctx.repo.database.store(&commit)?;
        self.ctx.repo.refs.update_head(&commit.oid())?;

        Ok(commit)
    }

    pub fn write_tree(&self) -> Tree {
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

    pub fn current_author(&self) -> Author {
        let name = &self.ctx.env["GIT_AUTHOR_NAME"];
        let email = &self.ctx.env["GIT_AUTHOR_EMAIL"];

        let author_date = if let Some(author_date_str) = self.ctx.env.get("GIT_AUTHOR_DATE") {
            DateTime::parse_from_rfc2822(author_date_str).expect("could not parse GIT_AUTHOR_DATE")
        } else {
            let now = Local::now();
            now.with_timezone(now.offset())
        };

        Author::new(name.clone(), email.clone(), author_date)
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

    pub fn resume_merge(&self, r#type: PendingCommitType) -> Result<()> {
        match r#type {
            PendingCommitType::Merge => self.write_merge_commit()?,
            PendingCommitType::CherryPick => self.write_cherry_pick_commit()?,
        }

        Err(Error::Exit(0))
    }

    fn write_merge_commit(&self) -> Result<()> {
        self.handle_conflicted_index()?;

        let parents = vec![
            self.ctx.repo.refs.read_head()?.unwrap(),
            self.pending_commit.merge_oid(PendingCommitType::Merge)?,
        ];
        let message = self.compose_merge_message(Some(MERGE_NOTES))?;
        self.write_commit(parents, message.as_deref())?;

        self.pending_commit.clear(PendingCommitType::Merge)?;

        Ok(())
    }

    pub fn write_cherry_pick_commit(&self) -> Result<()> {
        self.handle_conflicted_index()?;

        let parents = vec![self.ctx.repo.refs.read_head()?.unwrap()];
        let message = self.compose_merge_message(Some(CHERRY_PICK_NOTES))?;

        let pick_oid = self
            .pending_commit
            .merge_oid(PendingCommitType::CherryPick)?;
        let commit = self.ctx.repo.database.load_commit(&pick_oid)?;

        let picked = Commit::new(
            parents,
            self.write_tree().oid(),
            commit.author,
            self.current_author(),
            message.unwrap(),
        );

        self.ctx.repo.database.store(&picked)?;
        self.ctx.repo.refs.update_head(&picked.oid())?;
        self.pending_commit.clear(PendingCommitType::CherryPick)?;

        Ok(())
    }

    fn compose_merge_message(&self, notes: Option<&str>) -> Result<Option<String>> {
        self.ctx
            .edit_file(&self.commit_message_path(), |editor: &mut Editor| {
                editor.write(&self.pending_commit.merge_message()?)?;
                if let Some(notes) = notes {
                    editor.note(notes)?;
                }
                editor.write("")?;
                editor.note(COMMIT_NOTES)?;

                Ok(())
            })
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
