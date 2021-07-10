mod common;

use assert_cmd::assert::OutputAssertExt;
pub use common::CommandHelper;
use jit::database::object::Object;
use jit::errors::Result;
use rstest::{fixture, rstest};
use std::collections::HashMap;

fn commit_tree(
    helper: &mut CommandHelper,
    message: &str,
    files: HashMap<&str, &str>,
) -> Result<()> {
    for (path, contents) in files {
        helper.write_file(path, contents)?;
    }
    helper.jit_cmd(&["add", "."]);
    helper.commit(message);

    Ok(())
}

mod merging_an_ancestor {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut tree = HashMap::new();
        tree.insert("f.txt", "1");
        commit_tree(&mut helper, "A", tree).unwrap();

        let mut tree = HashMap::new();
        tree.insert("f.txt", "2");
        commit_tree(&mut helper, "B", tree).unwrap();

        let mut tree = HashMap::new();
        tree.insert("f.txt", "3");
        commit_tree(&mut helper, "C", tree).unwrap();

        helper
    }

    #[rstest]
    fn print_the_up_to_date_message_and_do_not_change_the_repository_state(
        mut helper: CommandHelper,
    ) -> Result<()> {
        helper
            .jit_cmd(&["merge", "@^"])
            .assert()
            .code(0)
            .stdout("Already up to date.\n");

        let commit = helper.load_commit("@")?;
        assert_eq!(commit.message, "C");

        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .code(0)
            .stdout("");

        Ok(())
    }
}

///   A   B   M
///   o---o---o
///    \     /
///     `---o
///         C
mod unconflicted_merge_with_two_files {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut tree = HashMap::new();
        tree.insert("f.txt", "1");
        tree.insert("g.txt", "1");
        commit_tree(&mut helper, "root", tree).unwrap();

        helper.jit_cmd(&["branch", "topic"]);
        helper.jit_cmd(&["checkout", "topic"]);
        let mut tree = HashMap::new();
        tree.insert("g.txt", "2");
        commit_tree(&mut helper, "right", tree).unwrap();

        helper.jit_cmd(&["checkout", "main"]);
        let mut tree = HashMap::new();
        tree.insert("f.txt", "2");
        commit_tree(&mut helper, "left", tree).unwrap();

        helper.stdin = String::from("merge topic branch");
        helper.jit_cmd(&["merge", "topic"]).assert().code(0);

        helper
    }

    #[rstest]
    fn put_the_combined_changes_in_the_workspace(helper: CommandHelper) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("f.txt", "2");
        workspace.insert("g.txt", "2");
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn leave_the_status_clean(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .code(0)
            .stdout("");
    }

    #[rstest]
    fn write_a_commit_with_the_old_head_and_the_merged_commit_as_parents(
        helper: CommandHelper,
    ) -> Result<()> {
        let commit = helper.load_commit("@")?;
        let old_head = helper.load_commit("@^")?;
        let merge_head = helper.load_commit("topic")?;

        assert_eq!(commit.parents, vec![old_head.oid(), merge_head.oid()]);

        Ok(())
    }
}

///   A   B   C       M1  H   M2
///   o---o---o-------o---o---o
///        \         /       /
///         o---o---o G     /
///         D  E \         /
///               `-------o
///                       F
mod multiple_common_ancestors {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut tree = HashMap::new();
        tree.insert("f.txt", "1");
        commit_tree(&mut helper, "A", tree).unwrap();
        let mut tree = HashMap::new();
        tree.insert("f.txt", "2");
        commit_tree(&mut helper, "B", tree).unwrap();
        let mut tree = HashMap::new();
        tree.insert("f.txt", "3");
        commit_tree(&mut helper, "C", tree).unwrap();

        helper.jit_cmd(&["branch", "topic", "main^"]);
        helper.jit_cmd(&["checkout", "topic"]);
        let mut tree = HashMap::new();
        tree.insert("g.txt", "1");
        commit_tree(&mut helper, "D", tree).unwrap();
        let mut tree = HashMap::new();
        tree.insert("g.txt", "2");
        commit_tree(&mut helper, "E", tree).unwrap();
        let mut tree = HashMap::new();
        tree.insert("g.txt", "3");
        commit_tree(&mut helper, "F", tree).unwrap();

        helper.jit_cmd(&["branch", "joiner", "topic^"]);
        helper.jit_cmd(&["checkout", "joiner"]);
        let mut tree = HashMap::new();
        tree.insert("h.txt", "1");
        commit_tree(&mut helper, "G", tree).unwrap();

        helper.jit_cmd(&["checkout", "main"]);

        helper
    }

    #[rstest]
    fn perform_the_first_merge(mut helper: CommandHelper) -> Result<()> {
        helper.stdin = String::from("merge joiner");
        helper.jit_cmd(&["merge", "joiner"]).assert().code(0);

        let mut workspace = HashMap::new();
        workspace.insert("f.txt", "3");
        workspace.insert("g.txt", "2");
        workspace.insert("h.txt", "1");
        helper.assert_workspace(&workspace)?;

        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .code(0)
            .stdout("");

        Ok(())
    }

    #[rstest]
    fn perform_the_second_merge(mut helper: CommandHelper) -> Result<()> {
        helper.stdin = String::from("merge joiner");
        helper.jit_cmd(&["merge", "joiner"]).assert().code(0);

        let mut tree = HashMap::new();
        tree.insert("f.txt", "4");
        commit_tree(&mut helper, "H", tree)?;

        helper.stdin = String::from("merge topic");
        helper.jit_cmd(&["merge", "topic"]).assert().code(0);

        let mut workspace = HashMap::new();
        workspace.insert("f.txt", "4");
        workspace.insert("g.txt", "3");
        workspace.insert("h.txt", "1");
        helper.assert_workspace(&workspace)?;

        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .code(0)
            .stdout("");

        Ok(())
    }
}
