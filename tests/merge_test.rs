mod common;

use assert_cmd::assert::OutputAssertExt;
pub use common::CommandHelper;
use jit::database::object::Object;
use jit::database::Database;
use jit::errors::Result;
use rstest::{fixture, rstest};
use std::collections::{BTreeMap, HashMap};

type Tree<'a> = BTreeMap<&'a str, Change<'a>>;

#[derive(Debug)]
struct Change<'a> {
    content: Option<&'a str>,
    executable: bool,
}

impl<'a> Change<'a> {
    pub fn content(content: &'a str) -> Self {
        Self {
            content: Some(content),
            executable: false,
        }
    }

    pub fn delete() -> Self {
        Self {
            content: None,
            executable: false,
        }
    }

    pub fn executable() -> Self {
        Self {
            content: None,
            executable: true,
        }
    }

    pub fn executable_content(content: &'a str) -> Self {
        Self {
            content: Some(content),
            executable: true,
        }
    }
}

fn commit_tree(helper: &mut CommandHelper, message: &str, files: Tree) -> Result<()> {
    for (path, change) in files {
        if !change.executable {
            // Delete `path` before writing to it in order to support replacing directories with files
            helper.force_delete(path)?;
        }

        if let Some(content) = change.content {
            helper.write_file(path, content)?;
        }

        if change.executable {
            helper.make_executable(path)?;
        }
    }
    helper.force_delete(".git/index")?;
    helper.jit_cmd(&["add", "."]);
    helper.commit(message);

    Ok(())
}

///   A   B   M
///   o---o---o [master]
///    \     /
///     `---o [topic]
///         C
///
fn merge3(helper: &mut CommandHelper, base: Tree, left: Tree, right: Tree) -> Result<()> {
    commit_tree(helper, "A", base)?;
    commit_tree(helper, "B", left)?;

    helper.jit_cmd(&["branch", "topic", "main^"]);
    helper.jit_cmd(&["checkout", "topic"]);
    commit_tree(helper, "C", right)?;

    helper.jit_cmd(&["checkout", "main"]);
    helper.stdin = String::from("M");
    helper.jit_cmd(&["merge", "topic"]);

    Ok(())
}

fn assert_clean_merge(helper: &mut CommandHelper) -> Result<()> {
    helper
        .jit_cmd(&["status", "--porcelain"])
        .assert()
        .code(0)
        .stdout("");

    let commit = helper.load_commit("@")?;
    let old_head = helper.load_commit("@^")?;
    let merge_head = helper.load_commit("topic")?;

    assert_eq!(commit.message, "M");
    assert_eq!(commit.parents, vec![old_head.oid(), merge_head.oid()]);

    Ok(())
}

fn assert_no_merge(helper: &mut CommandHelper) -> Result<()> {
    let commit = helper.load_commit("@")?;
    assert_eq!(commit.message, "B");
    assert_eq!(commit.parents.len(), 1);

    Ok(())
}

fn assert_index(helper: &mut CommandHelper, entries: Vec<(&str, u16)>) -> Result<()> {
    let mut repo = helper.repo();
    repo.index.load()?;

    let actual: Vec<_> = repo
        .index
        .entries
        .values()
        .map(|e| (e.path.as_str(), e.stage()))
        .collect();
    assert_eq!(actual, entries);

    Ok(())
}

mod merging_an_ancestor {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut tree = BTreeMap::new();
        tree.insert("f.txt", Change::content("1"));
        commit_tree(&mut helper, "A", tree).unwrap();

        let mut tree = BTreeMap::new();
        tree.insert("f.txt", Change::content("2"));
        commit_tree(&mut helper, "B", tree).unwrap();

        let mut tree = BTreeMap::new();
        tree.insert("f.txt", Change::content("3"));
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

mod fast_forward_merge {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut tree = BTreeMap::new();
        tree.insert("f.txt", Change::content("1"));
        commit_tree(&mut helper, "A", tree).unwrap();

        let mut tree = BTreeMap::new();
        tree.insert("f.txt", Change::content("2"));
        commit_tree(&mut helper, "B", tree).unwrap();

        let mut tree = BTreeMap::new();
        tree.insert("f.txt", Change::content("3"));
        commit_tree(&mut helper, "C", tree).unwrap();

        helper.jit_cmd(&["branch", "topic", "@^^"]);
        helper.jit_cmd(&["checkout", "topic"]);

        helper
    }

    #[rstest]
    fn print_the_fast_forward_message_and_update_the_current_branch_head(
        mut helper: CommandHelper,
    ) -> Result<()> {
        let a = helper.resolve_revision("main^^")?;
        let b = helper.resolve_revision("main")?;

        helper.stdin = String::from("M");
        helper
            .jit_cmd(&["merge", "main"])
            .assert()
            .code(0)
            .stdout(format!(
                "\
Updating {}..{}
Fast-forward
",
                Database::short_oid(&a),
                Database::short_oid(&b),
            ));

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

mod unconflicted_merge_with_two_files {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut base = BTreeMap::new();
        base.insert("f.txt", Change::content("1"));
        base.insert("g.txt", Change::content("1"));

        let mut left = BTreeMap::new();
        left.insert("f.txt", Change::content("2"));

        let mut right = BTreeMap::new();
        right.insert("g.txt", Change::content("2"));

        merge3(&mut helper, base, left, right).unwrap();

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
    fn create_a_clean_merge(mut helper: CommandHelper) -> Result<()> {
        assert_clean_merge(&mut helper)?;

        Ok(())
    }
}

mod unconflicted_merge_with_a_deleted_file {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut base = BTreeMap::new();
        base.insert("f.txt", Change::content("1"));
        base.insert("g.txt", Change::content("1"));

        let mut left = BTreeMap::new();
        left.insert("f.txt", Change::content("2"));

        let mut right = BTreeMap::new();
        right.insert("g.txt", Change::delete());

        merge3(&mut helper, base, left, right).unwrap();

        helper
    }

    #[rstest]
    fn put_the_combined_changes_in_the_workspace(helper: CommandHelper) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("f.txt", "2");
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn create_a_clean_merge(mut helper: CommandHelper) -> Result<()> {
        assert_clean_merge(&mut helper)?;

        Ok(())
    }
}

mod unconflicted_merge_same_addition_on_both_sides {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut base = BTreeMap::new();
        base.insert("f.txt", Change::content("1"));

        let mut left = BTreeMap::new();
        left.insert("g.txt", Change::content("2"));

        let mut right = BTreeMap::new();
        right.insert("g.txt", Change::content("2"));

        merge3(&mut helper, base, left, right).unwrap();

        helper
    }

    #[rstest]
    fn put_the_combined_changes_in_the_workspace(helper: CommandHelper) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("f.txt", "1");
        workspace.insert("g.txt", "2");
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn create_a_clean_merge(mut helper: CommandHelper) -> Result<()> {
        assert_clean_merge(&mut helper)?;

        Ok(())
    }
}

mod unconflicted_merge_same_edit_on_both_sides {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut base = BTreeMap::new();
        base.insert("f.txt", Change::content("1"));

        let mut left = BTreeMap::new();
        left.insert("f.txt", Change::content("2"));

        let mut right = BTreeMap::new();
        right.insert("f.txt", Change::content("2"));

        merge3(&mut helper, base, left, right).unwrap();

        helper
    }

    #[rstest]
    fn put_the_combined_changes_in_the_workspace(helper: CommandHelper) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("f.txt", "2");
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn create_a_clean_merge(mut helper: CommandHelper) -> Result<()> {
        assert_clean_merge(&mut helper)?;

        Ok(())
    }
}

mod unconflicted_merge_in_file_merge_possible {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut base = BTreeMap::new();
        base.insert("f.txt", Change::content("1\n2\n3\n"));

        let mut left = BTreeMap::new();
        left.insert("f.txt", Change::content("4\n2\n3\n"));

        let mut right = BTreeMap::new();
        right.insert("f.txt", Change::content("1\n2\n5\n"));

        merge3(&mut helper, base, left, right).unwrap();

        helper
    }

    #[rstest]
    fn put_the_combined_changes_in_the_workspace(helper: CommandHelper) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("f.txt", "4\n2\n5\n");
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn create_a_clean_merge(mut helper: CommandHelper) -> Result<()> {
        assert_clean_merge(&mut helper)?;

        Ok(())
    }
}

mod unconflicted_merge_edit_and_mode_change {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut base = BTreeMap::new();
        base.insert("f.txt", Change::content("1"));

        let mut left = BTreeMap::new();
        left.insert("f.txt", Change::content("2"));

        let mut right = BTreeMap::new();
        right.insert("f.txt", Change::executable());

        merge3(&mut helper, base, left, right).unwrap();

        helper
    }

    #[rstest]
    fn put_the_combined_changes_in_the_workspace(helper: CommandHelper) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("f.txt", "2");
        helper.assert_workspace(&workspace)?;
        helper.assert_executable("f.txt");

        Ok(())
    }

    #[rstest]
    fn create_a_clean_merge(mut helper: CommandHelper) -> Result<()> {
        assert_clean_merge(&mut helper)?;

        Ok(())
    }
}

mod unconflicted_merge_mode_change_and_edit {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut base = BTreeMap::new();
        base.insert("f.txt", Change::content("1"));

        let mut left = BTreeMap::new();
        left.insert("f.txt", Change::executable());

        let mut right = BTreeMap::new();
        right.insert("f.txt", Change::content("3"));

        merge3(&mut helper, base, left, right).unwrap();

        helper
    }

    #[rstest]
    fn put_the_combined_changes_in_the_workspace(helper: CommandHelper) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("f.txt", "3");
        helper.assert_workspace(&workspace)?;
        helper.assert_executable("f.txt");

        Ok(())
    }

    #[rstest]
    fn create_a_clean_merge(mut helper: CommandHelper) -> Result<()> {
        assert_clean_merge(&mut helper)?;

        Ok(())
    }
}

mod unconflicted_merge_same_deletion_on_both_sides {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut base = BTreeMap::new();
        base.insert("f.txt", Change::content("1"));
        base.insert("g.txt", Change::content("1"));

        let mut left = BTreeMap::new();
        left.insert("g.txt", Change::delete());

        let mut right = BTreeMap::new();
        right.insert("g.txt", Change::delete());

        merge3(&mut helper, base, left, right).unwrap();

        helper
    }

    #[rstest]
    fn put_the_combined_changes_in_the_workspace(helper: CommandHelper) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("f.txt", "1");
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn create_a_clean_merge(mut helper: CommandHelper) -> Result<()> {
        assert_clean_merge(&mut helper)?;

        Ok(())
    }
}

mod unconflicted_merge_delete_add_parent {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut base = BTreeMap::new();
        base.insert("nest/f.txt", Change::content("1"));

        let mut left = BTreeMap::new();
        left.insert("nest/f.txt", Change::delete());

        let mut right = BTreeMap::new();
        right.insert("nest", Change::content("3"));

        merge3(&mut helper, base, left, right).unwrap();

        helper
    }

    #[rstest]
    fn put_the_combined_changes_in_the_workspace(helper: CommandHelper) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("nest", "3");
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn create_a_clean_merge(mut helper: CommandHelper) -> Result<()> {
        assert_clean_merge(&mut helper)?;

        Ok(())
    }
}

mod unconflicted_merge_delete_add_child {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut base = BTreeMap::new();
        base.insert("nest/f.txt", Change::content("1"));

        let mut left = BTreeMap::new();
        left.insert("nest/f.txt", Change::delete());

        let mut right = BTreeMap::new();
        right.insert("nest", Change::delete());
        right.insert("nest/f.txt/g.txt", Change::content("3"));

        merge3(&mut helper, base, left, right).unwrap();

        helper
    }

    #[rstest]
    fn put_the_combined_changes_in_the_workspace(helper: CommandHelper) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("nest/f.txt/g.txt", "3");
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn create_a_clean_merge(mut helper: CommandHelper) -> Result<()> {
        assert_clean_merge(&mut helper)?;

        Ok(())
    }
}

mod conflicted_merge_add_add {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut base = BTreeMap::new();
        base.insert("f.txt", Change::content("1"));

        let mut left = BTreeMap::new();
        left.insert("g.txt", Change::content("2\n"));

        let mut right = BTreeMap::new();
        right.insert("g.txt", Change::content("3\n"));

        merge3(&mut helper, base, left, right).unwrap();

        helper
    }

    #[rstest]
    fn print_the_merge_conflicts(helper: CommandHelper) {
        helper.assert_stdout(
            "\
Auto-merging g.txt
CONFLICT (add/add): Merge conflict in g.txt
Automatic merge failed; fix conflicts and then commit the result.
",
        );
    }

    #[rstest]
    fn put_the_conflicted_file_in_the_workspace(helper: CommandHelper) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("f.txt", "1");
        workspace.insert(
            "g.txt",
            "\
<<<<<<< HEAD
2
=======
3
>>>>>>> topic
",
        );
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn record_the_conflict_in_the_index(mut helper: CommandHelper) -> Result<()> {
        assert_index(&mut helper, vec![("f.txt", 0), ("g.txt", 2), ("g.txt", 3)])?;

        Ok(())
    }

    #[rstest]
    fn do_not_write_a_merge_commit(mut helper: CommandHelper) -> Result<()> {
        assert_no_merge(&mut helper)?;

        Ok(())
    }

    #[rstest]
    fn report_the_conflict_in_the_status(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .code(0)
            .stdout("AA g.txt\n");
    }

    #[rstest]
    fn show_the_combined_diff_against_stages_2_and_3(mut helper: CommandHelper) {
        helper.jit_cmd(&["diff"]).assert().stdout(
            "\
diff --cc g.txt
index 0cfbf08,00750ed..2603ab2
--- a/g.txt
+++ b/g.txt
@@@ -1,1 -1,1 +1,5 @@@
++<<<<<<< HEAD
 +2
++=======
+ 3
++>>>>>>> topic
",
        );
    }

    #[rstest]
    fn show_the_diff_against_our_version(mut helper: CommandHelper) {
        helper.jit_cmd(&["diff", "--ours"]).assert().stdout(
            "\
* Unmerged path g.txt
diff --git a/g.txt b/g.txt
index 0cfbf08..2603ab2 100644
--- a/g.txt
+++ b/g.txt
@@ -1,1 +1,5 @@
+<<<<<<< HEAD
 2
+=======
+3
+>>>>>>> topic
",
        );
    }
    #[rstest]
    fn show_the_diff_against_their_version(mut helper: CommandHelper) {
        helper.jit_cmd(&["diff", "--theirs"]).assert().stdout(
            "\
* Unmerged path g.txt
diff --git a/g.txt b/g.txt
index 00750ed..2603ab2 100644
--- a/g.txt
+++ b/g.txt
@@ -1,1 +1,5 @@
+<<<<<<< HEAD
+2
+=======
 3
+>>>>>>> topic
",
        );
    }
}

mod conflicted_merge_add_add_mode_conflict {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut base = BTreeMap::new();
        base.insert("f.txt", Change::content("1"));

        let mut left = BTreeMap::new();
        left.insert("g.txt", Change::content("2"));

        let mut right = BTreeMap::new();
        right.insert("g.txt", Change::executable_content("2"));

        merge3(&mut helper, base, left, right).unwrap();

        helper
    }

    #[rstest]
    fn print_the_merge_conflicts(helper: CommandHelper) {
        helper.assert_stdout(
            "\
Auto-merging g.txt
CONFLICT (add/add): Merge conflict in g.txt
Automatic merge failed; fix conflicts and then commit the result.
",
        );
    }

    #[rstest]
    fn put_the_conflicted_file_in_the_workspace(helper: CommandHelper) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("f.txt", "1");
        workspace.insert("g.txt", "2");
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn record_the_conflict_in_the_index(mut helper: CommandHelper) -> Result<()> {
        assert_index(&mut helper, vec![("f.txt", 0), ("g.txt", 2), ("g.txt", 3)])?;

        Ok(())
    }

    #[rstest]
    fn do_not_write_a_merge_commit(mut helper: CommandHelper) -> Result<()> {
        assert_no_merge(&mut helper)?;

        Ok(())
    }

    #[rstest]
    fn report_the_conflict_in_the_status(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .code(0)
            .stdout("AA g.txt\n");
    }

    #[rstest]
    fn show_the_combined_diff_against_stages_2_and_3(mut helper: CommandHelper) {
        helper.jit_cmd(&["diff"]).assert().stdout(
            "\
diff --cc g.txt
index d8263ee,d8263ee..d8263ee
mode 100644,100755..100644
--- a/g.txt
+++ b/g.txt
",
        );
    }

    #[rstest]
    fn report_the_mode_change_in_the_appropriate_diff(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["diff", "-2"])
            .assert()
            .stdout("* Unmerged path g.txt\n");

        helper.jit_cmd(&["diff", "-3"]).assert().stdout(
            "\
* Unmerged path g.txt
diff --git a/g.txt b/g.txt
old mode 100755
new mode 100644
",
        );
    }
}

mod conflicted_merge_file_directory_addition {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut base = BTreeMap::new();
        base.insert("f.txt", Change::content("1"));

        let mut left = BTreeMap::new();
        left.insert("g.txt", Change::content("2"));

        let mut right = BTreeMap::new();
        right.insert("g.txt/nested.txt", Change::content("3"));

        merge3(&mut helper, base, left, right).unwrap();

        helper
    }

    #[rstest]
    fn print_the_merge_conflicts(helper: CommandHelper) {
        helper.assert_stdout(
            "\
Adding g.txt/nested.txt
CONFLICT (file/directory): There is a directory with name g.txt in topic. Adding g.txt as g.txt~HEAD
Automatic merge failed; fix conflicts and then commit the result.
",
        );
    }

    #[rstest]
    fn put_a_namespaced_copy_of_the_conflicted_file_in_the_workspace(
        helper: CommandHelper,
    ) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("f.txt", "1");
        workspace.insert("g.txt~HEAD", "2");
        workspace.insert("g.txt/nested.txt", "3");
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn record_the_conflict_in_the_index(mut helper: CommandHelper) -> Result<()> {
        assert_index(
            &mut helper,
            vec![("f.txt", 0), ("g.txt", 2), ("g.txt/nested.txt", 0)],
        )?;

        Ok(())
    }

    #[rstest]
    fn do_not_write_a_merge_commit(mut helper: CommandHelper) -> Result<()> {
        assert_no_merge(&mut helper)?;

        Ok(())
    }

    #[rstest]
    fn report_the_conflict_in_the_status(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .code(0)
            .stdout(
                "\
AU g.txt
A  g.txt/nested.txt
?? g.txt~HEAD
",
            );
    }

    #[rstest]
    fn list_the_files_as_unmerged_in_the_diff(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["diff"])
            .assert()
            .stdout("* Unmerged path g.txt\n");
    }
}

mod conflicted_merge_directory_file_addition {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut base = BTreeMap::new();
        base.insert("f.txt", Change::content("1"));

        let mut left = BTreeMap::new();
        left.insert("g.txt/nested.txt", Change::content("2"));

        let mut right = BTreeMap::new();
        right.insert("g.txt", Change::content("3"));

        merge3(&mut helper, base, left, right).unwrap();

        helper
    }

    #[rstest]
    fn print_the_merge_conflicts(helper: CommandHelper) {
        helper.assert_stdout(
            "\
Adding g.txt/nested.txt
CONFLICT (directory/file): There is a directory with name g.txt in HEAD. Adding g.txt as g.txt~topic
Automatic merge failed; fix conflicts and then commit the result.
",
        );
    }

    #[rstest]
    fn put_a_namespaced_copy_of_the_conflicted_file_in_the_workspace(
        helper: CommandHelper,
    ) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("f.txt", "1");
        workspace.insert("g.txt~topic", "3");
        workspace.insert("g.txt/nested.txt", "2");
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn record_the_conflict_in_the_index(mut helper: CommandHelper) -> Result<()> {
        assert_index(
            &mut helper,
            vec![("f.txt", 0), ("g.txt", 3), ("g.txt/nested.txt", 0)],
        )?;

        Ok(())
    }

    #[rstest]
    fn do_not_write_a_merge_commit(mut helper: CommandHelper) -> Result<()> {
        assert_no_merge(&mut helper)?;

        Ok(())
    }

    #[rstest]
    fn report_the_conflict_in_the_status(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .code(0)
            .stdout(
                "\
UA g.txt
?? g.txt~topic
",
            );
    }

    #[rstest]
    fn list_the_files_as_unmerged_in_the_diff(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["diff"])
            .assert()
            .stdout("* Unmerged path g.txt\n");
    }
}

mod conflicted_merge_edit_edit {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut base = BTreeMap::new();
        base.insert("f.txt", Change::content("1\n"));

        let mut left = BTreeMap::new();
        left.insert("f.txt", Change::content("2\n"));

        let mut right = BTreeMap::new();
        right.insert("f.txt", Change::content("3\n"));

        merge3(&mut helper, base, left, right).unwrap();

        helper
    }

    #[rstest]
    fn print_the_merge_conflicts(helper: CommandHelper) {
        helper.assert_stdout(
            "\
Auto-merging f.txt
CONFLICT (content): Merge conflict in f.txt
Automatic merge failed; fix conflicts and then commit the result.
",
        );
    }

    #[rstest]
    fn put_the_conflicted_file_in_the_workspace(helper: CommandHelper) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert(
            "f.txt",
            "\
<<<<<<< HEAD
2
=======
3
>>>>>>> topic
",
        );
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn record_the_conflict_in_the_index(mut helper: CommandHelper) -> Result<()> {
        assert_index(&mut helper, vec![("f.txt", 1), ("f.txt", 2), ("f.txt", 3)])?;

        Ok(())
    }

    #[rstest]
    fn do_not_write_a_merge_commit(mut helper: CommandHelper) -> Result<()> {
        assert_no_merge(&mut helper)?;

        Ok(())
    }

    #[rstest]
    fn report_the_conflict_in_the_status(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .code(0)
            .stdout("UU f.txt\n");
    }

    #[rstest]
    fn show_the_combined_diff_against_stages_2_and_3(mut helper: CommandHelper) {
        helper.jit_cmd(&["diff"]).assert().stdout(
            "\
diff --cc f.txt
index 0cfbf08,00750ed..2603ab2
--- a/f.txt
+++ b/f.txt
@@@ -1,1 -1,1 +1,5 @@@
++<<<<<<< HEAD
 +2
++=======
+ 3
++>>>>>>> topic
",
        );
    }
}

mod conflicted_merge_edit_delete {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut base = BTreeMap::new();
        base.insert("f.txt", Change::content("1"));

        let mut left = BTreeMap::new();
        left.insert("f.txt", Change::content("2"));

        let mut right = BTreeMap::new();
        right.insert("f.txt", Change::delete());

        merge3(&mut helper, base, left, right).unwrap();

        helper
    }

    #[rstest]
    fn print_the_merge_conflicts(helper: CommandHelper) {
        helper.assert_stdout(
            "\
CONFLICT (modify/delete): f.txt deleted in topic and modified in HEAD. Version HEAD of f.txt left in tree.
Automatic merge failed; fix conflicts and then commit the result.
");
    }

    #[rstest]
    fn put_the_left_version_in_the_workspace(helper: CommandHelper) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("f.txt", "2");
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn record_the_conflict_in_the_index(mut helper: CommandHelper) -> Result<()> {
        assert_index(&mut helper, vec![("f.txt", 1), ("f.txt", 2)])?;

        Ok(())
    }

    #[rstest]
    fn do_not_write_a_merge_commit(mut helper: CommandHelper) -> Result<()> {
        assert_no_merge(&mut helper)?;

        Ok(())
    }

    #[rstest]
    fn report_the_conflict_in_the_status(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .code(0)
            .stdout("UD f.txt\n");
    }

    #[rstest]
    fn list_the_files_as_unmerged_in_the_diff(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["diff"])
            .assert()
            .stdout("* Unmerged path f.txt\n");
    }
}

mod conflicted_merge_delete_edit {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut base = BTreeMap::new();
        base.insert("f.txt", Change::content("1"));

        let mut left = BTreeMap::new();
        left.insert("f.txt", Change::delete());

        let mut right = BTreeMap::new();
        right.insert("f.txt", Change::content("3"));

        merge3(&mut helper, base, left, right).unwrap();

        helper
    }

    #[rstest]
    fn print_the_merge_conflicts(helper: CommandHelper) {
        helper.assert_stdout(
            "\
CONFLICT (modify/delete): f.txt deleted in HEAD and modified in topic. Version topic of f.txt left in tree.
Automatic merge failed; fix conflicts and then commit the result.
");
    }

    #[rstest]
    fn put_the_right_version_in_the_workspace(helper: CommandHelper) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("f.txt", "3");
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn record_the_conflict_in_the_index(mut helper: CommandHelper) -> Result<()> {
        assert_index(&mut helper, vec![("f.txt", 1), ("f.txt", 3)])?;

        Ok(())
    }

    #[rstest]
    fn do_not_write_a_merge_commit(mut helper: CommandHelper) -> Result<()> {
        assert_no_merge(&mut helper)?;

        Ok(())
    }

    #[rstest]
    fn report_the_conflict_in_the_status(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .code(0)
            .stdout("DU f.txt\n");
    }

    #[rstest]
    fn list_the_files_as_unmerged_in_the_diff(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["diff"])
            .assert()
            .stdout("* Unmerged path f.txt\n");
    }
}

mod conflicted_merge_edit_add_parent {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut base = BTreeMap::new();
        base.insert("nest/f.txt", Change::content("1"));

        let mut left = BTreeMap::new();
        left.insert("nest/f.txt", Change::content("2"));

        let mut right = BTreeMap::new();
        right.insert("nest", Change::content("3"));

        merge3(&mut helper, base, left, right).unwrap();

        helper
    }

    #[rstest]
    fn print_the_merge_conflicts(helper: CommandHelper) {
        helper.assert_stdout("\
CONFLICT (modify/delete): nest/f.txt deleted in topic and modified in HEAD. Version HEAD of nest/f.txt left in tree.
CONFLICT (directory/file): There is a directory with name nest in HEAD. Adding nest as nest~topic
Automatic merge failed; fix conflicts and then commit the result.
");
    }

    #[rstest]
    fn put_a_namespaced_copy_of_the_conflicted_file_in_the_workspace(
        helper: CommandHelper,
    ) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("nest/f.txt", "2");
        workspace.insert("nest~topic", "3");
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn record_the_conflict_in_the_index(mut helper: CommandHelper) -> Result<()> {
        assert_index(
            &mut helper,
            vec![("nest", 3), ("nest/f.txt", 1), ("nest/f.txt", 2)],
        )?;

        Ok(())
    }

    #[rstest]
    fn do_not_write_a_merge_commit(mut helper: CommandHelper) -> Result<()> {
        assert_no_merge(&mut helper)?;

        Ok(())
    }

    #[rstest]
    fn report_the_conflict_in_the_status(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .code(0)
            .stdout(
                "\
UA nest
UD nest/f.txt
?? nest~topic
",
            );
    }

    #[rstest]
    fn list_the_files_as_unmerged_in_the_diff(mut helper: CommandHelper) {
        helper.jit_cmd(&["diff"]).assert().stdout(
            "\
* Unmerged path nest
* Unmerged path nest/f.txt
",
        );
    }
}

mod conflicted_merge_edit_add_child {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut base = BTreeMap::new();
        base.insert("nest/f.txt", Change::content("1"));

        let mut left = BTreeMap::new();
        left.insert("nest/f.txt", Change::content("2"));

        let mut right = BTreeMap::new();
        right.insert("nest/f.txt", Change::delete());
        right.insert("nest/f.txt/g.txt", Change::content("3"));

        merge3(&mut helper, base, left, right).unwrap();

        helper
    }

    #[rstest]
    fn print_the_merge_conflicts(helper: CommandHelper) {
        helper.assert_stdout("\
Adding nest/f.txt/g.txt
CONFLICT (modify/delete): nest/f.txt deleted in topic and modified in HEAD. Version HEAD of nest/f.txt left in tree at nest/f.txt~HEAD.
Automatic merge failed; fix conflicts and then commit the result.
");
    }

    #[rstest]
    fn put_a_namespaced_copy_of_the_conflicted_file_in_the_workspace(
        helper: CommandHelper,
    ) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("nest/f.txt~HEAD", "2");
        workspace.insert("nest/f.txt/g.txt", "3");
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn record_the_conflict_in_the_index(mut helper: CommandHelper) -> Result<()> {
        assert_index(
            &mut helper,
            vec![
                ("nest/f.txt", 1), // missing
                ("nest/f.txt", 2),
                ("nest/f.txt/g.txt", 0),
            ],
        )?;

        Ok(())
    }

    #[rstest]
    fn do_not_write_a_merge_commit(mut helper: CommandHelper) -> Result<()> {
        assert_no_merge(&mut helper)?;

        Ok(())
    }

    #[rstest]
    fn report_the_conflict_in_the_status(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .code(0)
            .stdout(
                "\
UD nest/f.txt
A  nest/f.txt/g.txt
?? nest/f.txt~HEAD
",
            );
    }

    #[rstest]
    fn list_the_files_as_unmerged_in_the_diff(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["diff"])
            .assert()
            .stdout("* Unmerged path nest/f.txt\n");
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

        let mut tree = BTreeMap::new();
        tree.insert("f.txt", Change::content("1"));
        commit_tree(&mut helper, "A", tree).unwrap();
        let mut tree = BTreeMap::new();
        tree.insert("f.txt", Change::content("2"));
        commit_tree(&mut helper, "B", tree).unwrap();
        let mut tree = BTreeMap::new();
        tree.insert("f.txt", Change::content("3"));
        commit_tree(&mut helper, "C", tree).unwrap();

        helper.jit_cmd(&["branch", "topic", "main^"]);
        helper.jit_cmd(&["checkout", "topic"]);
        let mut tree = BTreeMap::new();
        tree.insert("g.txt", Change::content("1"));
        commit_tree(&mut helper, "D", tree).unwrap();
        let mut tree = BTreeMap::new();
        tree.insert("g.txt", Change::content("2"));
        commit_tree(&mut helper, "E", tree).unwrap();
        let mut tree = BTreeMap::new();
        tree.insert("g.txt", Change::content("3"));
        commit_tree(&mut helper, "F", tree).unwrap();

        helper.jit_cmd(&["branch", "joiner", "topic^"]);
        helper.jit_cmd(&["checkout", "joiner"]);
        let mut tree = BTreeMap::new();
        tree.insert("h.txt", Change::content("1"));
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

        let mut tree = BTreeMap::new();
        tree.insert("f.txt", Change::content("4"));
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

mod conflict_resolution {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut base = BTreeMap::new();
        base.insert("f.txt", Change::content("1"));

        let mut left = BTreeMap::new();
        left.insert("f.txt", Change::content("2"));

        let mut right = BTreeMap::new();
        right.insert("f.txt", Change::content("3"));

        merge3(&mut helper, base, left, right).unwrap();

        helper
    }

    #[rstest]
    fn prevent_commits_with_unmerged_entries(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["commit"]).assert().code(128).stderr(
            "\
error: Committing is not possible because you have unmerged files.
hint: Fix them up in the work tree, and then use 'jit add <file>'
hint: as appropriate to mark resolution and make a commit.
fatal: Exiting because of an unresolved conflict.
",
        );

        assert_eq!(helper.load_commit("@")?.message, "B");

        Ok(())
    }

    #[rstest]
    fn prevent_merge_continue_with_unmerged_entries(mut helper: CommandHelper) -> Result<()> {
        helper
            .jit_cmd(&["merge", "--continue"])
            .assert()
            .code(128)
            .stderr(
                "\
error: Committing is not possible because you have unmerged files.
hint: Fix them up in the work tree, and then use 'jit add <file>'
hint: as appropriate to mark resolution and make a commit.
fatal: Exiting because of an unresolved conflict.
",
            );

        assert_eq!(helper.load_commit("@")?.message, "B");

        Ok(())
    }

    #[rstest]
    fn commit_a_merge_after_resolving_conflicts(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["add", "f.txt"]);
        helper.jit_cmd(&["commit"]).assert().code(0);

        let commit = helper.load_commit("@")?;
        assert_eq!(commit.message, "M");

        let parents: Vec<_> = commit
            .parents
            .iter()
            .map(|oid| helper.load_commit(&oid).unwrap().message)
            .collect();
        assert_eq!(parents, vec!["B", "C"]);

        Ok(())
    }

    #[rstest]
    fn allow_merge_continue_after_resolving_conflicts(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["add", "f.txt"]);
        helper.jit_cmd(&["merge", "--continue"]).assert().code(0);

        let commit = helper.load_commit("@")?;
        assert_eq!(commit.message, "M");

        let parents: Vec<_> = commit
            .parents
            .iter()
            .map(|oid| helper.load_commit(&oid).unwrap().message)
            .collect();
        assert_eq!(parents, vec!["B", "C"]);

        Ok(())
    }

    #[rstest]
    fn prevent_merge_continue_when_none_is_in_progress(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["add", "f.txt"]);
        helper.jit_cmd(&["merge", "--continue"]);
        helper
            .jit_cmd(&["merge", "--continue"])
            .assert()
            .code(128)
            .stderr("fatal: There is no merge in progress (MERGE_HEAD missing).\n");

        Ok(())
    }

    #[rstest]
    fn prevent_starting_a_new_merge_while_one_is_in_progress(
        mut helper: CommandHelper,
    ) -> Result<()> {
        helper.jit_cmd(&["merge"]).assert().code(128).stderr(
            "\
error: Merging is not possible because you have unmerged files.
hint: Fix them up in the work tree, and then use 'jit add <file>'
hint: as appropriate to mark resolution and make a commit.
fatal: Exiting because of an unresolved conflict.
",
        );

        Ok(())
    }
}
