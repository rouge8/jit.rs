mod common;

pub use common::CommandHelper;
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
}
