mod common;

pub use common::CommandHelper;
use jit::errors::Result;
use rstest::{fixture, rstest};

mod committing_to_branches {
    use super::*;

    fn commit_change(helper: &mut CommandHelper, content: &'static str) -> Result<()> {
        helper.write_file("file.txt", content)?;
        helper.jit_cmd(&["add", "."]);
        helper.commit(content);

        Ok(())
    }

    #[fixture]
    fn base_helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        for message in ["first", "second", "third"] {
            helper.write_file("file.txt", message).unwrap();
            helper.jit_cmd(&["add", "."]);
            helper.commit(message);
        }

        helper.jit_cmd(&["branch", "topic"]);
        helper.jit_cmd(&["checkout", "topic"]);

        helper
    }

    mod on_a_branch {
        use super::*;

        #[fixture]
        fn helper(base_helper: CommandHelper) -> CommandHelper {
            base_helper
        }

        #[rstest]
        fn advance_a_branch_pointer(mut helper: CommandHelper) -> Result<()> {
            let repo = helper.repo();

            let head_before = repo.refs.read_ref("HEAD")?;

            commit_change(&mut helper, "change")?;

            let head_after = repo.refs.read_ref("HEAD")?;
            let branch_after = repo.refs.read_ref("topic")?;

            assert_ne!(head_after, head_before);
            assert_eq!(branch_after, head_after);

            assert_eq!(helper.resolve_revision("@^")?, head_before.unwrap());

            // The lock should be rolled back
            assert!(!helper.repo_path.join(".git/HEAD.lock").exists());

            Ok(())
        }

        #[rstest]
        #[case("Wed, 27 May 2020 09:40:54 -0700", "Wed May 27 09:40:54 2020 -0700")]
        #[case("Mon, 28 Jun 2021 17:41:12 +1000", "Mon Jun 28 17:41:12 2021 +1000")]
        fn parse_git_author_date(
            #[case] input: &'static str,
            #[case] expected: &'static str,
            mut helper: CommandHelper,
        ) -> Result<()> {
            helper
                .env
                .insert(String::from("GIT_AUTHOR_DATE"), String::from(input));
            commit_change(&mut helper, "change")?;

            let commit = helper.load_commit("@")?;
            assert_eq!(commit.author.readable_time(), expected);

            Ok(())
        }
    }

    mod with_a_detached_head {
        use super::*;

        #[fixture]
        fn helper(mut base_helper: CommandHelper) -> CommandHelper {
            base_helper.jit_cmd(&["checkout", "@"]);

            base_helper
        }

        #[rstest]
        fn advance_head(mut helper: CommandHelper) -> Result<()> {
            let repo = helper.repo();

            let head_before = repo.refs.read_ref("HEAD")?;
            commit_change(&mut helper, "change")?;
            let head_after = repo.refs.read_ref("HEAD")?;

            assert_ne!(head_after, head_before);

            Ok(())
        }

        #[rstest]
        fn do_not_advance_the_detached_branch(mut helper: CommandHelper) -> Result<()> {
            let repo = helper.repo();

            let branch_before = repo.refs.read_ref("topic")?;
            commit_change(&mut helper, "change")?;
            let branch_after = repo.refs.read_ref("topic")?;

            assert_eq!(branch_after, branch_before);

            Ok(())
        }

        #[rstest]
        fn leave_head_a_commit_ahead_of_the_branch(mut helper: CommandHelper) -> Result<()> {
            commit_change(&mut helper, "change")?;

            assert_eq!(
                helper.resolve_revision("@^")?,
                helper.repo().refs.read_ref("topic")?.unwrap(),
            );

            Ok(())
        }
    }

    mod with_concurrent_branches {
        use super::*;

        #[fixture]
        fn helper(mut base_helper: CommandHelper) -> CommandHelper {
            base_helper.jit_cmd(&["branch", "fork", "@^"]);

            base_helper
        }

        #[rstest]
        fn advance_the_branches_from_a_shared_parent(mut helper: CommandHelper) -> Result<()> {
            commit_change(&mut helper, "A")?;
            commit_change(&mut helper, "B")?;

            helper.jit_cmd(&["checkout", "fork"]);
            commit_change(&mut helper, "C")?;

            assert_ne!(
                helper.resolve_revision("fork")?,
                helper.resolve_revision("topic")?,
            );

            assert_eq!(
                helper.resolve_revision("fork^")?,
                helper.resolve_revision("topic~3")?,
            );

            Ok(())
        }
    }
}
