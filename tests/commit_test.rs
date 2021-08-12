mod common;

use assert_cmd::assert::OutputAssertExt;
pub use common::CommandHelper;
use jit::database::object::Object;
use jit::database::tree_diff::Differ;
use jit::errors::Result;
use jit::rev_list::RevList;
use jit::util::path_to_string;
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
            let head_before = helper.repo.refs.read_ref("HEAD")?;

            commit_change(&mut helper, "change")?;

            let head_after = helper.repo.refs.read_ref("HEAD")?;
            let branch_after = helper.repo.refs.read_ref("topic")?;

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
            let head_before = helper.repo.refs.read_ref("HEAD")?;
            commit_change(&mut helper, "change")?;
            let head_after = helper.repo.refs.read_ref("HEAD")?;

            assert_ne!(head_after, head_before);

            Ok(())
        }

        #[rstest]
        fn do_not_advance_the_detached_branch(mut helper: CommandHelper) -> Result<()> {
            let branch_before = helper.repo.refs.read_ref("topic")?;
            commit_change(&mut helper, "change")?;
            let branch_after = helper.repo.refs.read_ref("topic")?;

            assert_eq!(branch_after, branch_before);

            Ok(())
        }

        #[rstest]
        fn leave_head_a_commit_ahead_of_the_branch(mut helper: CommandHelper) -> Result<()> {
            commit_change(&mut helper, "change")?;

            assert_eq!(
                helper.resolve_revision("@^")?,
                helper.repo.refs.read_ref("topic")?.unwrap(),
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

mod reusing_messages {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.write_file("file.txt", "1").unwrap();
        helper.jit_cmd(&["add", "."]);
        helper.commit("first");

        helper
    }

    #[rstest]
    fn use_the_message_from_another_commit(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("file.txt", "2")?;
        helper.jit_cmd(&["add", "."]);
        helper.jit_cmd(&["commit", "-C", "@"]).assert().code(0);

        let revs = RevList::new(&helper.repo, &[String::from("HEAD")])?;
        assert_eq!(
            revs.map(|commit| commit.message.trim().to_owned())
                .collect::<Vec<_>>(),
            vec![String::from("first"), String::from("first")]
        );

        Ok(())
    }
}

mod amending_commits {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        for message in ["first", "second", "third"] {
            helper.write_file("file.txt", message).unwrap();
            helper.jit_cmd(&["add", "."]);
            helper.commit(message);
        }

        helper
    }

    #[rstest]
    fn replace_the_last_commits_message(mut helper: CommandHelper) -> Result<()> {
        helper
            .jit_cmd(&["commit", "--amend", "--message", "third [amended]"])
            .assert()
            .code(0);
        let revs = RevList::new(&helper.repo, &[String::from("HEAD")])?;

        assert_eq!(
            revs.map(|commit| commit.message.trim().to_owned())
                .collect::<Vec<_>>(),
            vec![
                String::from("third [amended]"),
                String::from("second"),
                String::from("first")
            ]
        );

        Ok(())
    }

    #[rstest]
    fn replace_the_last_commits_tree(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("another.txt", "1")?;
        helper.jit_cmd(&["add", "another.txt"]);
        helper.jit_cmd(&["commit", "--amend"]).assert().code(0);

        let commit = helper.load_commit("HEAD")?;
        let diff = helper.repo.database.tree_diff(
            commit.parent().as_deref(),
            Some(&commit.oid()),
            None,
        )?;

        assert_eq!(
            diff.keys()
                .map(|path| path_to_string(path))
                .collect::<Vec<_>>(),
            vec![String::from("file.txt"), String::from("another.txt")],
        );

        Ok(())
    }
}
