use crate::commands::shared::commit_writer::CommitWriter;
use crate::commands::{Command, CommandContext};
use crate::database::tree_diff::Differ;
use crate::database::Database;
use crate::errors::{Error, Result};
use crate::merge::inputs::Inputs;
use crate::merge::resolve::Resolve;
use crate::revision::HEAD;
use std::io;
use std::io::Read;

pub struct Merge<'a> {
    ctx: CommandContext<'a>,
    inputs: Inputs,
}

impl<'a> Merge<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Result<Self> {
        let args = match &ctx.opt.cmd {
            Command::Merge { args } => args,
            _ => unreachable!(),
        };

        let inputs = Inputs::new(&ctx.repo, HEAD.to_string(), args[0].clone())?;

        Ok(Self { ctx, inputs })
    }

    pub fn run(&mut self) -> Result<()> {
        if self.inputs.already_merged() {
            self.handle_merged_ancestor()?;
        }
        if self.inputs.is_fast_forward() {
            self.handle_fast_forward()?;
        }

        self.resolve_merge()?;
        self.commit_merge()?;

        Ok(())
    }

    fn resolve_merge(&mut self) -> Result<()> {
        self.ctx.repo.index.load_for_update()?;

        let mut merge = Resolve::new(&mut self.ctx.repo, &self.inputs);
        merge.execute()?;

        self.ctx.repo.index.write_updates()?;
        if self.ctx.repo.index.has_conflict() {
            return Err(Error::Exit(1));
        }

        Ok(())
    }

    fn commit_merge(&self) -> Result<()> {
        let parents = vec![self.inputs.left_oid.clone(), self.inputs.right_oid.clone()];

        let mut message = String::new();
        io::stdin().read_to_string(&mut message)?;
        message = message.trim().to_string();

        self.commit_writer().write_commit(parents, message)?;

        Ok(())
    }

    fn handle_merged_ancestor(&self) -> Result<()> {
        let mut stdout = self.ctx.stdout.borrow_mut();

        writeln!(stdout, "Already up to date.")?;

        Err(Error::Exit(0))
    }

    fn handle_fast_forward(&mut self) -> Result<()> {
        let a = Database::short_oid(&self.inputs.left_oid);
        let b = Database::short_oid(&self.inputs.right_oid);

        let mut stdout = self.ctx.stdout.borrow_mut();
        writeln!(stdout, "Updating {}..{}", a, b)?;
        writeln!(stdout, "Fast-forward")?;

        self.ctx.repo.index.load_for_update()?;

        let tree_diff = self.ctx.repo.database.tree_diff(
            Some(&self.inputs.left_oid),
            Some(&self.inputs.right_oid),
            None,
        )?;
        self.ctx.repo.migration(tree_diff).apply_changes()?;

        self.ctx.repo.index.write_updates()?;
        self.ctx.repo.refs.update_head(&self.inputs.right_oid)?;

        Err(Error::Exit(0))
    }

    fn commit_writer(&self) -> CommitWriter {
        CommitWriter::new(&self.ctx)
    }
}
