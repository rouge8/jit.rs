mod common;

pub use common::CommandHelper;
use jit::database::ParsedObject;
use jit::errors::Result;
use rstest::{fixture, rstest};

mod with_a_chain_of_commits {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let messages = ["first", "second", "third"];

        for message in messages {
            helper.write_file("file.txt", message).unwrap();
            helper.jit_cmd(&["add", "."]);
            helper.commit(message);
        }

        helper
    }

    #[rstest]
    fn create_a_branch_pointing_at_head(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["branch", "topic"]);

        let repo = helper.repo();
        assert_eq!(repo.refs.read_ref("topic")?, repo.refs.read_head()?);

        Ok(())
    }

    #[rstest]
    fn fail_for_invalid_branch_name(mut helper: CommandHelper) {
        let cmd = helper.jit_cmd(&["branch", "^"]);

        assert_eq!(cmd.status.code().unwrap(), 128);
        assert_eq!(
            String::from_utf8(cmd.stderr).unwrap(),
            "fatal: '^' is not a valid branch name.\n"
        );
    }

    #[rstest]
    fn fail_for_existing_branch_name(mut helper: CommandHelper) {
        helper.jit_cmd(&["branch", "topic"]);
        let cmd = helper.jit_cmd(&["branch", "topic"]);

        assert_eq!(cmd.status.code().unwrap(), 128);
        assert_eq!(
            String::from_utf8(cmd.stderr).unwrap(),
            "fatal: A branch named 'topic' already exists.\n"
        );
    }

    #[rstest]
    fn create_a_branch_pointing_at_heads_parent(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["branch", "topic", "HEAD^"]);

        let mut repo = helper.repo();

        let head = repo.database.load(&repo.refs.read_head()?.unwrap())?;
        let head = match head {
            ParsedObject::Commit(commit) => commit,
            _ => unreachable!(),
        };

        assert_eq!(
            &repo.refs.read_ref("topic")?.unwrap(),
            head.parent.as_ref().unwrap(),
        );

        Ok(())
    }

    #[rstest]
    fn create_a_branch_pointing_at_heads_grandparent(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["branch", "topic", "@~2"]);

        let mut repo = helper.repo();
        let head = repo.database.load(&repo.refs.read_head()?.unwrap())?;
        let head = match head {
            ParsedObject::Commit(commit) => commit,
            _ => unreachable!(),
        };

        let mut repo = helper.repo();
        let parent = repo.database.load(head.parent.as_ref().unwrap())?;
        let parent = match parent {
            ParsedObject::Commit(commit) => commit,
            _ => unreachable!(),
        };

        assert_eq!(
            &repo.refs.read_ref("topic")?.unwrap(),
            parent.parent.as_ref().unwrap(),
        );

        Ok(())
    }

    #[rstest]
    fn create_a_branch_relative_to_another_one(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["branch", "topic", "@~1"]);
        helper.jit_cmd(&["branch", "another", "topic^"]);

        let repo = helper.repo();
        assert_eq!(
            repo.refs.read_ref("another")?.unwrap(),
            helper.resolve_revision("HEAD~2")?,
        );

        Ok(())
    }

    #[rstest]
    fn fail_for_invalid_revisions(mut helper: CommandHelper) {
        let cmd = helper.jit_cmd(&["branch", "topic", "^"]);

        assert_eq!(cmd.status.code().unwrap(), 128);
        assert_eq!(
            String::from_utf8(cmd.stderr).unwrap(),
            "fatal: Not a valid object name: '^'.\n",
        );
    }

    #[rstest]
    fn fail_for_invalid_refs(mut helper: CommandHelper) {
        let cmd = helper.jit_cmd(&["branch", "topic", "no-such-branch"]);

        assert_eq!(cmd.status.code().unwrap(), 128);
        assert_eq!(
            String::from_utf8(cmd.stderr).unwrap(),
            "fatal: Not a valid object name: 'no-such-branch'.\n",
        );
    }

    #[rstest]
    fn fail_for_invalid_parents(mut helper: CommandHelper) {
        let cmd = helper.jit_cmd(&["branch", "topic", "@^^^^"]);

        assert_eq!(cmd.status.code().unwrap(), 128);
        assert_eq!(
            String::from_utf8(cmd.stderr).unwrap(),
            "fatal: Not a valid object name: '@^^^^'.\n",
        );
    }

    #[rstest]
    fn fail_for_invalid_ancestors(mut helper: CommandHelper) {
        let cmd = helper.jit_cmd(&["branch", "topic", "@~50"]);

        assert_eq!(cmd.status.code().unwrap(), 128);
        assert_eq!(
            String::from_utf8(cmd.stderr).unwrap(),
            "fatal: Not a valid object name: '@~50'.\n",
        );
    }
}
