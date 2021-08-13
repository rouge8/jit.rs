use crate::commands::shared::commit_writer::CommitWriter;
use crate::commands::{Command, CommandContext};
use crate::database::commit::Commit;
use crate::database::object::Object;
use crate::database::Database;
use crate::errors::Result;
use crate::merge::inputs;
use crate::merge::resolve::Resolve;
use crate::refs::HEAD;
use crate::revision::{Revision, COMMIT};

pub struct CherryPick<'a> {
    ctx: CommandContext<'a>,
    revision: String,
}

impl<'a> CherryPick<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Self {
        let revision = match &ctx.opt.cmd {
            Command::CherryPick { revision } => revision.to_owned(),
            _ => unreachable!(),
        };

        Self { ctx, revision }
    }

    pub fn run(&mut self) -> Result<()> {
        let mut revision = Revision::new(&self.ctx.repo, &self.revision);
        let commit = self
            .ctx
            .repo
            .database
            .load_commit(&revision.resolve(Some(COMMIT))?)?;

        self.pick(&commit)?;

        Ok(())
    }

    fn pick(&mut self, commit: &Commit) -> Result<()> {
        let inputs = self.pick_merge_inputs(commit)?;

        self.resolve_merge(&inputs)?;

        let commit_writer = self.commit_writer();

        let picked = Commit::new(
            vec![inputs.left_oid],
            commit_writer.write_tree().oid(),
            commit.author.clone(),
            commit_writer.current_author(),
            commit.message.clone(),
        );

        self.finish_commit(&commit_writer, &picked)?;

        Ok(())
    }

    fn pick_merge_inputs(&self, commit: &Commit) -> Result<inputs::CherryPick> {
        let short = Database::short_oid(&commit.oid());

        let left_name = HEAD.to_owned();
        let left_oid = self.ctx.repo.refs.read_head()?.unwrap();

        let right_name = format!("{}... {}", short, commit.title_line().trim());
        let right_oid = commit.oid();

        Ok(inputs::CherryPick::new(
            left_name,
            right_name,
            left_oid,
            right_oid,
            vec![commit.parent().unwrap()],
        ))
    }

    fn resolve_merge(&mut self, inputs: &inputs::CherryPick) -> Result<()> {
        self.ctx.repo.index.load_for_update()?;
        Resolve::new(&mut self.ctx.repo, inputs).execute()?;
        self.ctx.repo.index.write_updates()?;

        Ok(())
    }

    fn finish_commit(&self, commit_writer: &CommitWriter, commit: &Commit) -> Result<()> {
        self.ctx.repo.database.store(commit)?;
        self.ctx.repo.refs.update_head(&commit.oid())?;
        commit_writer.print_commit(commit)?;

        Ok(())
    }

    fn commit_writer(&self) -> CommitWriter {
        CommitWriter::new(&self.ctx)
    }
}
