mod common;

use assert_cmd::prelude::OutputAssertExt;
pub use common::CommandHelper;
use jit::errors::Result;
use jit::refs::Ref;
use lazy_static::lazy_static;
use rstest::{fixture, rstest};
use std::collections::HashMap;
use std::process::Output;

mod with_a_set_of_files {
    use super::*;

    lazy_static! {
        static ref BASE_FILES: HashMap<&'static str, &'static str> = {
            let mut m = HashMap::new();
            m.insert("1.txt", "1");
            m.insert("outer/2.txt", "2");
            m.insert("outer/inner/3.txt", "3");

            m
        };
    }

    fn commit_all(helper: &mut CommandHelper) -> Result<()> {
        helper.delete(".git/index")?;
        helper.jit_cmd(&["add", "."]);
        helper.commit("change");

        Ok(())
    }

    fn commit_and_checkout(helper: &mut CommandHelper, revision: &str) -> Result<()> {
        commit_all(helper)?;
        helper.jit_cmd(&["checkout", revision]).assert().code(0);

        Ok(())
    }

    fn assert_stale_file(output: Output, filename: &str) {
        output
            .assert()
            .stderr(format!(
                "\
error: Your local changes to the following files would be overwritten by checkout:
\t{}
Please commit your changes or stash them before you switch branches.
Aborting\n",
                filename
            ))
            .code(1);
    }

    fn assert_stale_directory(output: Output, filename: &str) {
        output
            .assert()
            .stderr(format!(
                "\
error: Updating the following directories would lose untracked files in them:
\t{}

Aborting\n",
                filename
            ))
            .code(1);
    }

    fn assert_overwrite_conflict(output: Output, filename: &str) {
        output
            .assert()
            .stderr(format!(
                "\
error: The following untracked working tree files would be overwritten by checkout:
\t{}
Please move or remove them before you switch branches.
Aborting\n",
                filename
            ))
            .code(1);
    }

    fn assert_remove_conflict(output: Output, filename: &str) {
        output
            .assert()
            .stderr(format!(
                "\
error: The following untracked working tree files would be removed by checkout:
\t{}
Please move or remove them before you switch branches.
Aborting\n",
                filename
            ))
            .code(1);
    }

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        for (name, contents) in BASE_FILES.iter() {
            helper.write_file(name, contents).unwrap();
        }
        helper.jit_cmd(&["add", "."]);
        helper.commit("first");

        helper
    }

    #[rstest]
    fn update_a_changed_file(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("1.txt", "changed")?;
        commit_and_checkout(&mut helper, "@^")?;

        helper.assert_workspace(&*BASE_FILES)?;
        helper.assert_status("");

        Ok(())
    }

    #[rstest]
    fn remove_a_file(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("94.txt", "94")?;
        commit_and_checkout(&mut helper, "@^")?;

        helper.assert_workspace(&*BASE_FILES)?;
        helper.assert_status("");

        Ok(())
    }

    #[rstest]
    fn remove_a_file_from_an_existing_directory(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/94.txt", "94")?;
        commit_and_checkout(&mut helper, "@^")?;

        helper.assert_workspace(&*BASE_FILES)?;
        helper.assert_status("");

        Ok(())
    }

    #[rstest]
    fn remove_a_file_from_a_new_directory(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("new/94.txt", "94")?;
        commit_and_checkout(&mut helper, "@^")?;

        helper.assert_workspace(&*BASE_FILES)?;
        helper.assert_status("");

        Ok(())
    }

    #[rstest]
    fn remove_a_file_from_a_new_nested_directory(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("new/inner/94.txt", "94")?;
        commit_and_checkout(&mut helper, "@^")?;

        helper.assert_workspace(&*BASE_FILES)?;
        helper.assert_status("");

        Ok(())
    }

    #[rstest]
    fn remove_a_file_from_a_non_empty_directory(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/94.txt", "94")?;
        commit_and_checkout(&mut helper, "@^")?;

        helper.assert_workspace(&*BASE_FILES)?;
        helper.assert_status("");

        Ok(())
    }

    #[rstest]
    fn add_a_file(mut helper: CommandHelper) -> Result<()> {
        helper.delete("1.txt")?;
        commit_and_checkout(&mut helper, "@^")?;

        helper.assert_workspace(&*BASE_FILES)?;
        helper.assert_status("");

        Ok(())
    }

    #[rstest]
    fn add_a_file_to_a_directory(mut helper: CommandHelper) -> Result<()> {
        helper.delete("outer/2.txt")?;
        commit_and_checkout(&mut helper, "@^")?;

        helper.assert_workspace(&*BASE_FILES)?;
        helper.assert_status("");

        Ok(())
    }

    #[rstest]
    fn replace_a_file_with_a_directory(mut helper: CommandHelper) -> Result<()> {
        helper.delete("outer/inner")?;
        helper.write_file("outer/inner", "in")?;
        commit_and_checkout(&mut helper, "@^")?;

        helper.assert_workspace(&*BASE_FILES)?;
        helper.assert_status("");

        Ok(())
    }

    #[rstest]
    fn replace_a_directory_with_a_file(mut helper: CommandHelper) -> Result<()> {
        helper.delete("outer/2.txt")?;
        helper.write_file("outer/2.txt/nested.log", "nested")?;
        commit_and_checkout(&mut helper, "@^")?;

        helper.assert_workspace(&*BASE_FILES)?;
        helper.assert_status("");

        Ok(())
    }

    #[rstest]
    fn maintain_workspace_modifications(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("1.txt", "changed")?;
        commit_all(&mut helper)?;

        helper.write_file("outer/2.txt", "hello")?;
        helper.delete("outer/inner")?;
        helper.jit_cmd(&["checkout", "@^"]);

        let mut expected = HashMap::new();
        expected.insert("1.txt", "1");
        expected.insert("outer/2.txt", "hello");
        helper.assert_workspace(&expected)?;

        helper.assert_status(
            " M outer/2.txt
 D outer/inner/3.txt\n",
        );

        Ok(())
    }

    #[rstest]
    fn maintain_index_modifications(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("1.txt", "changed")?;
        commit_all(&mut helper)?;

        helper.write_file("outer/2.txt", "hello")?;
        helper.write_file("outer/inner/4.txt", "world")?;
        helper.jit_cmd(&["add", "."]);
        helper.jit_cmd(&["checkout", "@^"]);

        let mut expected = BASE_FILES.clone();
        expected.insert("outer/2.txt", "hello");
        expected.insert("outer/inner/4.txt", "world");
        helper.assert_workspace(&expected)?;

        helper.assert_status(
            "M  outer/2.txt
A  outer/inner/4.txt\n",
        );

        Ok(())
    }

    #[rstest]
    fn fail_to_update_a_modified_file(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("1.txt", "changed")?;
        commit_all(&mut helper)?;

        helper.write_file("1.txt", "conflict")?;

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_stale_file(output, "1.txt");

        Ok(())
    }

    #[rstest]
    fn fail_to_update_a_modified_equal_file(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("1.txt", "changed")?;
        commit_all(&mut helper)?;

        helper.write_file("1.txt", "1")?;

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_stale_file(output, "1.txt");

        Ok(())
    }

    #[rstest]
    fn fail_to_update_a_changed_mode_file(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("1.txt", "changed")?;
        commit_all(&mut helper)?;

        helper.make_executable("1.txt")?;

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_stale_file(output, "1.txt");

        Ok(())
    }

    #[rstest]
    fn restore_a_deleted_file(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("1.txt", "changed")?;
        commit_all(&mut helper)?;

        helper.delete("1.txt")?;
        helper.jit_cmd(&["checkout", "@^"]);

        helper.assert_workspace(&*BASE_FILES)?;
        helper.assert_status("");

        Ok(())
    }

    #[rstest]
    fn restore_files_from_a_deleted_directory(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/inner/3.txt", "changed")?;
        commit_all(&mut helper)?;

        helper.delete("outer")?;
        helper.jit_cmd(&["checkout", "@^"]);

        let mut expected = HashMap::new();
        expected.insert("1.txt", "1");
        expected.insert("outer/inner/3.txt", "3");
        helper.assert_workspace(&expected)?;

        helper.assert_status(" D outer/2.txt\n");

        Ok(())
    }

    #[rstest]
    fn fail_to_update_a_staged_file(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("1.txt", "changed")?;
        commit_all(&mut helper)?;

        helper.write_file("1.txt", "conflict")?;
        helper.jit_cmd(&["add", "."]);

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_stale_file(output, "1.txt");

        Ok(())
    }

    #[rstest]
    fn update_a_staged_equal_file(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("1.txt", "changed")?;
        commit_all(&mut helper)?;

        helper.write_file("1.txt", "1")?;
        helper.jit_cmd(&["add", "."]);
        helper.jit_cmd(&["checkout", "@^"]);

        helper.assert_workspace(&*BASE_FILES)?;
        helper.assert_status("");

        Ok(())
    }

    #[rstest]
    fn fail_to_update_a_staged_changed_mode_file(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("1.txt", "changed")?;
        commit_all(&mut helper)?;

        helper.make_executable("1.txt")?;
        helper.jit_cmd(&["add", "."]);

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_stale_file(output, "1.txt");

        Ok(())
    }

    #[rstest]
    fn fail_to_update_an_unindexed_file(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("1.txt", "changed")?;
        commit_all(&mut helper)?;

        helper.delete("1.txt")?;
        helper.delete(".git/index")?;
        helper.jit_cmd(&["add", "."]);

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_stale_file(output, "1.txt");

        Ok(())
    }

    #[rstest]
    fn fail_to_update_an_unindexed_and_untracked_file(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("1.txt", "changed")?;
        commit_all(&mut helper)?;

        helper.delete("1.txt")?;
        helper.delete(".git/index")?;
        helper.jit_cmd(&["add", "."]);
        helper.write_file("1.txt", "conflict")?;

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_stale_file(output, "1.txt");

        Ok(())
    }

    #[rstest]
    fn fail_to_update_an_unindexed_directory(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/inner/3.txt", "changed")?;
        commit_all(&mut helper)?;

        helper.delete("outer/inner")?;
        helper.delete(".git/index")?;
        helper.jit_cmd(&["add", "."]);

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_stale_file(output, "outer/inner/3.txt");

        Ok(())
    }

    #[rstest]
    fn fail_to_update_with_a_file_at_a_parent_path(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/inner/3.txt", "changed")?;
        commit_all(&mut helper)?;

        helper.delete("outer/inner")?;
        helper.write_file("outer/inner", "conflict")?;

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_stale_file(output, "outer/inner/3.txt");

        Ok(())
    }

    #[rstest]
    fn fail_to_update_with_a_staged_file_at_a_parent_path(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/inner/3.txt", "changed")?;
        commit_all(&mut helper)?;

        helper.delete("outer/inner")?;
        helper.write_file("outer/inner", "conflict")?;
        helper.jit_cmd(&["add", "."]);

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_stale_file(output, "outer/inner/3.txt");

        Ok(())
    }

    #[rstest]
    fn fail_to_update_with_an_unstaged_file_at_a_parent_path(
        mut helper: CommandHelper,
    ) -> Result<()> {
        helper.write_file("outer/inner/3.txt", "changed")?;
        commit_all(&mut helper)?;

        helper.delete("outer/inner")?;
        helper.delete(".git/index")?;
        helper.jit_cmd(&["add", "."]);
        helper.write_file("outer/inner", "conflict")?;

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_stale_file(output, "outer/inner/3.txt");

        Ok(())
    }

    #[rstest]
    fn fail_to_update_with_a_file_at_a_child_path(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/2.txt", "changed")?;
        commit_all(&mut helper)?;

        helper.delete("outer/2.txt")?;
        helper.write_file("outer/2.txt/extra.log", "conflict")?;

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_stale_file(output, "outer/2.txt");

        Ok(())
    }

    #[rstest]
    fn fail_to_update_with_a_staged_file_at_a_child_path(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/2.txt", "changed")?;
        commit_all(&mut helper)?;

        helper.delete("outer/2.txt")?;
        helper.write_file("outer/2.txt/extra.log", "conflict")?;
        helper.jit_cmd(&["add", "."]);

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_stale_file(output, "outer/2.txt");

        Ok(())
    }

    #[rstest]
    fn fail_to_remove_a_modified_file(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/94.txt", "94")?;
        commit_all(&mut helper)?;

        helper.write_file("outer/94.txt", "conflict")?;

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_stale_file(output, "outer/94.txt");

        Ok(())
    }

    #[rstest]
    fn fail_to_remove_a_changed_mode_file(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/94.txt", "94")?;
        commit_all(&mut helper)?;

        helper.make_executable("outer/94.txt")?;

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_stale_file(output, "outer/94.txt");

        Ok(())
    }

    #[rstest]
    fn leave_a_deleted_file_deleted(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/94.txt", "94")?;
        commit_all(&mut helper)?;

        helper.delete("outer/94.txt")?;
        helper.jit_cmd(&["checkout", "@^"]);

        helper.assert_workspace(&*BASE_FILES)?;
        helper.assert_status("");

        Ok(())
    }

    #[rstest]
    fn leave_a_deleted_directory_deleted(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/inner/94.txt", "94")?;
        commit_all(&mut helper)?;

        helper.delete("outer/inner")?;
        helper.jit_cmd(&["checkout", "@^"]);

        let mut expected = HashMap::new();
        expected.insert("1.txt", "1");
        expected.insert("outer/2.txt", "2");
        helper.assert_workspace(&expected)?;

        helper.assert_status(" D outer/inner/3.txt\n");

        Ok(())
    }

    #[rstest]
    fn fail_to_remove_a_staged_file(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/94.txt", "94")?;
        commit_all(&mut helper)?;

        helper.write_file("outer/94.txt", "conflict")?;
        helper.jit_cmd(&["add", "."]);

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_stale_file(output, "outer/94.txt");

        Ok(())
    }

    #[rstest]
    fn fail_to_remove_a_staged_changed_mode_file(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/94.txt", "94")?;
        commit_all(&mut helper)?;

        helper.make_executable("outer/94.txt")?;
        helper.jit_cmd(&["add", "."]);

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_stale_file(output, "outer/94.txt");

        Ok(())
    }

    #[rstest]
    fn leave_an_unindexed_file_deleted(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/94.txt", "94")?;
        commit_all(&mut helper)?;

        helper.delete("outer/94.txt")?;
        helper.delete(".git/index")?;
        helper.jit_cmd(&["add", "."]);
        helper.jit_cmd(&["checkout", "@^"]);

        helper.assert_workspace(&*BASE_FILES)?;
        helper.assert_status("");

        Ok(())
    }

    #[rstest]
    fn fail_to_remove_an_unindexed_and_untracked_file(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/94.txt", "94")?;
        commit_all(&mut helper)?;

        helper.delete("outer/94.txt")?;
        helper.delete(".git/index")?;
        helper.jit_cmd(&["add", "."]);
        helper.write_file("outer/94.txt", "conflict")?;

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_remove_conflict(output, "outer/94.txt");

        Ok(())
    }

    #[rstest]
    fn leave_an_unindexed_directory_deleted(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/inner/94.txt", "94")?;
        commit_all(&mut helper)?;

        helper.delete("outer/inner")?;
        helper.delete(".git/index")?;
        helper.jit_cmd(&["add", "."]);
        helper.jit_cmd(&["checkout", "@^"]);

        let mut expected = HashMap::new();
        expected.insert("1.txt", "1");
        expected.insert("outer/2.txt", "2");
        helper.assert_workspace(&expected)?;

        helper.assert_status("D  outer/inner/3.txt\n");

        Ok(())
    }

    #[rstest]
    fn fail_to_remove_with_a_file_at_a_parent_path(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/inner/94.txt", "94")?;
        commit_all(&mut helper)?;

        helper.delete("outer/inner")?;
        helper.write_file("outer/inner", "conflict")?;

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_stale_file(output, "outer/inner/94.txt");

        Ok(())
    }

    #[rstest]
    fn remove_a_file_with_a_staged_file_at_a_parent_path(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/inner/94.txt", "94")?;
        commit_all(&mut helper)?;

        helper.delete("outer/inner")?;
        helper.write_file("outer/inner", "conflict")?;
        helper.jit_cmd(&["add", "."]);
        helper.jit_cmd(&["checkout", "@^"]);

        let mut expected = HashMap::new();
        expected.insert("1.txt", "1");
        expected.insert("outer/2.txt", "2");
        expected.insert("outer/inner", "conflict");
        helper.assert_workspace(&expected)?;

        helper.assert_status(
            "\
A  outer/inner
D  outer/inner/3.txt\n",
        );

        Ok(())
    }

    #[rstest]
    fn fail_to_remove_with_an_unstaged_file_at_a_parent_path(
        mut helper: CommandHelper,
    ) -> Result<()> {
        helper.write_file("outer/inner/94.txt", "94")?;
        commit_all(&mut helper)?;

        helper.delete("outer/inner")?;
        helper.delete(".git/index")?;
        helper.jit_cmd(&["add", "."]);
        helper.write_file("outer/inner", "conflict")?;

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_remove_conflict(output, "outer/inner");

        Ok(())
    }

    #[rstest]
    fn fail_to_remove_with_a_file_at_a_child_path(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/94.txt", "94")?;
        commit_all(&mut helper)?;

        helper.delete("outer/94.txt")?;
        helper.write_file("outer/94.txt/extra.log", "conflict")?;

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_stale_file(output, "outer/94.txt");

        Ok(())
    }

    #[rstest]
    fn remove_a_file_with_a_staged_file_at_a_child_path(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/94.txt", "94")?;
        commit_all(&mut helper)?;

        helper.delete("outer/94.txt")?;
        helper.write_file("outer/94.txt/extra.log", "conflict")?;
        helper.jit_cmd(&["add", "."]);
        helper.jit_cmd(&["checkout", "@^"]);

        helper.assert_workspace(&*BASE_FILES)?;
        helper.assert_status("");

        Ok(())
    }

    #[rstest]
    fn fail_to_add_an_untracked_file(mut helper: CommandHelper) -> Result<()> {
        helper.delete("outer/2.txt")?;
        commit_all(&mut helper)?;

        helper.write_file("outer/2.txt", "conflict")?;

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_overwrite_conflict(output, "outer/2.txt");

        Ok(())
    }

    #[rstest]
    fn fail_to_add_an_added_file(mut helper: CommandHelper) -> Result<()> {
        helper.delete("outer/2.txt")?;
        commit_all(&mut helper)?;

        helper.write_file("outer/2.txt", "conflict")?;
        helper.jit_cmd(&["add", "."]);

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_stale_file(output, "outer/2.txt");

        Ok(())
    }

    #[rstest]
    fn add_a_staged_equal_file(mut helper: CommandHelper) -> Result<()> {
        helper.delete("outer/2.txt")?;
        commit_all(&mut helper)?;

        helper.write_file("outer/2.txt", "2")?;
        helper.jit_cmd(&["add", "."]);
        helper.jit_cmd(&["checkout", "@^"]);

        helper.assert_workspace(&*BASE_FILES)?;
        helper.assert_status("");

        Ok(())
    }

    #[rstest]
    fn fail_to_add_with_an_untracked_file_at_a_parent_path(
        mut helper: CommandHelper,
    ) -> Result<()> {
        helper.delete("outer/inner/3.txt")?;
        commit_all(&mut helper)?;

        helper.delete("outer/inner")?;
        helper.write_file("outer/inner", "conflict")?;

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_overwrite_conflict(output, "outer/inner");

        Ok(())
    }

    #[rstest]
    fn add_a_file_with_an_added_file_at_a_parent_path(mut helper: CommandHelper) -> Result<()> {
        helper.delete("outer/inner/3.txt")?;
        commit_all(&mut helper)?;

        helper.delete("outer/inner")?;
        helper.write_file("outer/inner", "conflict")?;
        helper.jit_cmd(&["add", "."]);
        helper.jit_cmd(&["checkout", "@^"]);

        helper.assert_workspace(&*BASE_FILES)?;
        helper.assert_status("");

        Ok(())
    }

    #[rstest]
    fn fail_to_add_with_an_untracked_file_at_a_child_path(mut helper: CommandHelper) -> Result<()> {
        helper.delete("outer/2.txt")?;
        commit_all(&mut helper)?;

        helper.write_file("outer/2.txt/extra.log", "conflict")?;

        let output = helper.jit_cmd(&["checkout", "@^"]);
        assert_stale_directory(output, "outer/2.txt");

        Ok(())
    }

    #[rstest]
    fn add_a_file_with_an_added_file_at_a_child_path(mut helper: CommandHelper) -> Result<()> {
        helper.delete("outer/2.txt")?;
        commit_all(&mut helper)?;

        helper.write_file("outer/2.txt/extra.log", "conflict")?;
        helper.jit_cmd(&["add", "."]);
        helper.jit_cmd(&["checkout", "@^"]);

        helper.assert_workspace(&*BASE_FILES)?;
        helper.assert_status("");

        Ok(())
    }
}

mod with_a_chain_of_commits {
    use super::*;

    #[fixture]
    fn base_helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        for message in ["first", "second", "third"] {
            helper.write_file("file.txt", "message").unwrap();
            helper.jit_cmd(&["add", "."]);
            helper.commit(message);
        }

        helper.jit_cmd(&["branch", "topic"]);
        helper.jit_cmd(&["branch", "second", "@^"]);

        helper
    }

    mod checking_out_a_branch {
        use super::*;

        #[fixture]
        fn helper(mut base_helper: CommandHelper) -> CommandHelper {
            base_helper.jit_cmd(&["checkout", "topic"]);

            base_helper
        }

        #[rstest]
        fn link_head_to_the_branch(helper: CommandHelper) -> Result<()> {
            let path = match helper.repo().refs.current_ref("HEAD")? {
                Ref::SymRef { path } => path,
                _ => unreachable!(),
            };
            assert_eq!(path, "refs/heads/topic");

            Ok(())
        }

        #[rstest]
        fn resolve_head_to_the_same_object_as_the_branch(helper: CommandHelper) -> Result<()> {
            let repo = helper.repo();
            assert_eq!(repo.refs.read_head()?, repo.refs.read_ref("topic")?);

            Ok(())
        }
    }

    mod checking_out_a_relative_revision {
        use super::*;

        #[fixture]
        fn helper(mut base_helper: CommandHelper) -> CommandHelper {
            base_helper.jit_cmd(&["checkout", "topic^"]);

            base_helper
        }

        #[rstest]
        fn detach_head(helper: CommandHelper) -> Result<()> {
            let path = match helper.repo().refs.current_ref("HEAD")? {
                Ref::SymRef { path } => path,
                _ => unreachable!(),
            };
            assert_eq!(path, "HEAD");

            Ok(())
        }

        #[rstest]
        fn put_the_revision_value_in_head(helper: CommandHelper) -> Result<()> {
            assert_eq!(
                helper.repo().refs.read_head()?,
                Some(helper.resolve_revision("topic^")?),
            );

            Ok(())
        }
    }
}
