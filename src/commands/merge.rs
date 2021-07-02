use crate::commands::shared::commit_writer::CommitWriter;
use crate::commands::{Command, CommandContext};
use crate::database::tree_diff::Differ;
use crate::errors::Result;
use crate::merge::common_ancestors::CommonAncestors;
use crate::revision::{Revision, COMMIT};
use std::io;
use std::io::Read;

pub struct Merge<'a> {
    ctx: CommandContext<'a>,
    args: Vec<String>,
}

impl<'a> Merge<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Self {
        let args = match &ctx.opt.cmd {
            Command::Merge { args } => args,
            _ => unreachable!(),
        };

        Self {
            ctx,
            args: args.to_owned(),
        }
    }

    pub fn run(&mut self) -> Result<()> {
        let head_oid = self.ctx.repo.refs.read_head()?.unwrap();
        let mut revision = Revision::new(&self.ctx.repo, &self.args[0]);
        let merge_oid = revision.resolve(Some(COMMIT))?;

        let mut common = CommonAncestors::new(&self.ctx.repo.database, &head_oid, &merge_oid)?;
        let base_oid = common.find()?;

        self.ctx.repo.index.load_for_update()?;

        let tree_diff =
            self.ctx
                .repo
                .database
                .tree_diff(base_oid.as_deref(), Some(&merge_oid), None)?;
        let mut migration = self.ctx.repo.migration(tree_diff);
        migration.apply_changes()?;

        self.ctx.repo.index.write_updates()?;

        let mut message = String::new();
        io::stdin().read_to_string(&mut message)?;
        message = message.trim().to_string();

        self.commit_writer()
            .write_commit(vec![head_oid, merge_oid], message)?;

        Ok(())
    }

    fn commit_writer(&self) -> CommitWriter {
        CommitWriter::new(&self.ctx)
    }
}
