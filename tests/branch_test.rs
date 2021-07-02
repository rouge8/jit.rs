mod common;

use assert_cmd::prelude::OutputAssertExt;
pub use common::CommandHelper;
use jit::database::object::Object;
use jit::database::Database;
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
        helper
            .jit_cmd(&["branch", "^"])
            .assert()
            .code(128)
            .stderr("fatal: '^' is not a valid branch name.\n");
    }

    #[rstest]
    fn fail_for_existing_branch_name(mut helper: CommandHelper) {
        helper.jit_cmd(&["branch", "topic"]);
        helper
            .jit_cmd(&["branch", "topic"])
            .assert()
            .code(128)
            .stderr("fatal: A branch named 'topic' already exists.\n");
    }

    #[rstest]
    fn create_a_branch_pointing_at_heads_parent(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["branch", "topic", "HEAD^"]);

        let repo = helper.repo();

        let head = repo
            .database
            .load_commit(&repo.refs.read_head()?.unwrap())?;

        assert_eq!(
            &repo.refs.read_ref("topic")?.unwrap(),
            head.parent().as_ref().unwrap(),
        );

        Ok(())
    }

    #[rstest]
    fn create_a_branch_pointing_at_heads_grandparent(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["branch", "topic", "@~2"]);

        let repo = helper.repo();
        let head = repo
            .database
            .load_commit(&repo.refs.read_head()?.unwrap())?;

        let repo = helper.repo();
        let parent = repo.database.load_commit(head.parent().as_ref().unwrap())?;

        assert_eq!(
            &repo.refs.read_ref("topic")?.unwrap(),
            parent.parent().as_ref().unwrap(),
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
    fn create_a_branch_from_a_short_commit_id(mut helper: CommandHelper) -> Result<()> {
        let repo = helper.repo();

        let commit_id = helper.resolve_revision("@~2")?;
        helper.jit_cmd(&["branch", "topic", &Database::short_oid(&commit_id)]);

        assert_eq!(repo.refs.read_ref("topic")?.unwrap(), commit_id);

        Ok(())
    }

    #[rstest]
    fn fail_for_invalid_revisions(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["branch", "topic", "^"])
            .assert()
            .code(128)
            .stderr("fatal: Not a valid object name: '^'.\n");
    }

    #[rstest]
    fn fail_for_invalid_refs(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["branch", "topic", "no-such-branch"])
            .assert()
            .code(128)
            .stderr("fatal: Not a valid object name: 'no-such-branch'.\n");
    }

    #[rstest]
    fn fail_for_invalid_parents(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["branch", "topic", "@^^^^"])
            .assert()
            .code(128)
            .stderr("fatal: Not a valid object name: '@^^^^'.\n");
    }

    #[rstest]
    fn fail_for_invalid_ancestors(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["branch", "topic", "@~50"])
            .assert()
            .code(128)
            .stderr("fatal: Not a valid object name: '@~50'.\n");
    }

    #[rstest]
    fn fail_for_revisions_that_are_not_commits(mut helper: CommandHelper) -> Result<()> {
        let repo = helper.repo();
        let tree_id = repo
            .database
            .load_commit(&repo.refs.read_head()?.unwrap())?
            .tree;

        helper
            .jit_cmd(&["branch", "topic", &tree_id])
            .assert()
            .code(128)
            .stderr(format!(
                "\
error: object {} is a tree, not a commit
fatal: Not a valid object name: '{}'.
",
                tree_id, tree_id,
            ));

        Ok(())
    }

    #[rstest]
    fn fail_for_parents_of_revisions_that_are_not_commits(mut helper: CommandHelper) -> Result<()> {
        let repo = helper.repo();
        let tree_id = repo
            .database
            .load_commit(&repo.refs.read_head()?.unwrap())?
            .tree;

        helper
            .jit_cmd(&["branch", "topic", &format!("{}^^", tree_id)])
            .assert()
            .code(128)
            .stderr(format!(
                "\
error: object {} is a tree, not a commit
fatal: Not a valid object name: '{}^^'.
",
                tree_id, tree_id,
            ));

        Ok(())
    }

    #[rstest]
    fn list_existing_branches(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["branch", "new-feature"]);

        helper.jit_cmd(&["branch"]).assert().code(0).stdout(
            "\
* main
  new-feature\n",
        );

        Ok(())
    }

    #[rstest]
    fn list_existing_branches_with_verbose_info(mut helper: CommandHelper) -> Result<()> {
        let a = helper.load_commit("@^")?;
        let b = helper.load_commit("@")?;

        helper.jit_cmd(&["branch", "new-feature", "@^"]);

        helper
            .jit_cmd(&["branch", "--verbose"])
            .assert()
            .code(0)
            .stdout(format!(
                "\
* main        {} third
  new-feature {} second\n",
                Database::short_oid(&b.oid()),
                Database::short_oid(&a.oid())
            ));

        Ok(())
    }

    #[rstest]
    fn delete_a_branch(mut helper: CommandHelper) -> Result<()> {
        let repo = helper.repo();

        let head = repo.refs.read_head()?.unwrap();

        helper.jit_cmd(&["branch", "bug-fix"]);

        helper
            .jit_cmd(&["branch", "-D", "bug-fix"])
            .assert()
            .code(0)
            .stdout(format!(
                "Deleted branch bug-fix (was {}).\n",
                Database::short_oid(&head)
            ));

        let branches: Vec<_> = repo
            .refs
            .list_branches()?
            .iter()
            .map(|r#ref| repo.refs.short_name(&r#ref))
            .collect();
        assert_eq!(branches, vec![String::from("main")]);

        Ok(())
    }

    #[rstest]
    fn delete_the_empty_parent_directories_of_a_branch(mut helper: CommandHelper) -> Result<()> {
        let repo = helper.repo();

        let head = repo.refs.read_head()?.unwrap();

        helper.jit_cmd(&["branch", "fix/bug/1"]);
        helper.jit_cmd(&["branch", "fix/2"]);

        helper
            .jit_cmd(&["branch", "-D", "fix/bug/1"])
            .assert()
            .code(0)
            .stdout(format!(
                "Deleted branch fix/bug/1 (was {}).\n",
                Database::short_oid(&head)
            ));

        let mut branches: Vec<_> = repo
            .refs
            .list_branches()?
            .iter()
            .map(|r#ref| repo.refs.short_name(&r#ref))
            .collect();
        branches.sort();
        assert_eq!(branches, vec![String::from("fix/2"), String::from("main")]);

        // The empty parent directory was deleted
        assert!(!helper.repo_path.join(".git/refs/heads/fix/bug").exists());

        Ok(())
    }

    #[rstest]
    fn fail_to_delete_a_non_existent_branch(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["branch", "-D", "no-such-branch"])
            .assert()
            .code(1)
            .stderr("error: branch 'no-such-branch' not found.\n");
    }
}
