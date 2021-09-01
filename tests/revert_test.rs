mod common;

use assert_cmd::prelude::OutputAssertExt;
pub use common::CommandHelper;
use jit::database::object::Object;
use jit::database::Database;
use jit::errors::Result;
use jit::rev_list::RevList;
use rstest::{fixture, rstest};
use std::collections::HashMap;

fn commit_tree(
    helper: &mut CommandHelper,
    message: &str,
    files: &HashMap<&str, &str>,
) -> Result<()> {
    for (path, contents) in files {
        helper.write_file(path, contents)?;
    }
    helper.jit_cmd(&["add", "."]);
    helper.commit(message);

    Ok(())
}

#[fixture]
fn base_helper() -> CommandHelper {
    let mut helper = CommandHelper::new();
    helper.init();

    for message in ["one", "two", "three", "four"] {
        let mut tree = HashMap::new();
        tree.insert("f.txt", message);
        commit_tree(&mut helper, message, &tree).unwrap();
    }

    let mut tree = HashMap::new();
    tree.insert("g.txt", "five");
    commit_tree(&mut helper, "five", &tree).unwrap();

    let mut tree = HashMap::new();
    tree.insert("f.txt", "six");
    commit_tree(&mut helper, "six", &tree).unwrap();

    let mut tree = HashMap::new();
    tree.insert("g.txt", "seven");
    commit_tree(&mut helper, "seven", &tree).unwrap();

    let mut tree = HashMap::new();
    tree.insert("g.txt", "eight");
    commit_tree(&mut helper, "eight", &tree).unwrap();

    helper
}

mod with_a_chain_of_commits {
    use super::*;

    #[fixture]
    fn helper(base_helper: CommandHelper) -> CommandHelper {
        base_helper
    }

    #[rstest]
    fn revert_a_commit_on_top_of_the_current_head(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["revert", "@~2"]).assert().code(0);

        let revs = RevList::new(&helper.repo, &[String::from("@~3..")], Default::default())?;

        assert_eq!(
            revs.map(|commit| commit.title_line().trim().to_owned())
                .collect::<Vec<_>>(),
            vec![
                String::from("Revert \"six\""),
                String::from("eight"),
                String::from("seven")
            ]
        );

        let mut tree = HashMap::new();
        tree.insert("f.txt", "four");
        tree.insert("g.txt", "eight");

        helper.assert_index(&tree)?;

        helper.assert_workspace(&tree)?;

        Ok(())
    }

    #[rstest]
    fn fail_to_revert_a_content_conflict(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["revert", "@~4"]).assert().code(1);

        let short = Database::short_oid(&helper.resolve_revision("@~4")?);

        let mut workspace = HashMap::new();
        workspace.insert("g.txt", "eight");
        let conflict = format!(
            "\
<<<<<<< HEAD
six=======
three>>>>>>> parent of {}... four
",
            short
        );
        workspace.insert("f.txt", &conflict);
        helper.assert_workspace(&workspace)?;

        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .stdout("UU f.txt\n");

        Ok(())
    }

    #[rstest]
    fn fail_to_revert_a_modify_delete_conflict(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["revert", "@~3"]).assert().code(1);

        let mut workspace = HashMap::new();
        workspace.insert("f.txt", "six");
        workspace.insert("g.txt", "eight");
        helper.assert_workspace(&workspace)?;

        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .stdout("UD g.txt\n");

        Ok(())
    }

    #[rstest]
    fn continue_a_conflicted_revert(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["revert", "@~3"]);
        helper.jit_cmd(&["add", "g.txt"]);

        helper
            .jit_cmd(&["revert", "--continue"])
            .assert()
            .code(0)
            // TODO: Remove
            .stderr("");

        let commits: Vec<_> =
            RevList::new(&helper.repo, &[String::from("@~3..")], Default::default())?.collect();
        assert_eq!(vec![commits[1].oid()], commits[0].parents);

        assert_eq!(
            commits
                .iter()
                .map(|commit| commit.title_line().trim().to_owned())
                .collect::<Vec<_>>(),
            vec![
                String::from("Revert \"five\""),
                String::from("eight"),
                String::from("seven")
            ]
        );

        let mut tree = HashMap::new();
        tree.insert("f.txt", "six");
        tree.insert("g.txt", "eight");

        helper.assert_index(&tree)?;

        helper.assert_workspace(&tree)?;

        Ok(())
    }

    #[rstest]
    fn commit_after_a_conflicted_revert(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["revert", "@~3"]);
        helper.jit_cmd(&["add", "g.txt"]);

        helper.jit_cmd(&["commit"]).assert().code(0);

        let commits: Vec<_> =
            RevList::new(&helper.repo, &[String::from("@~3..")], Default::default())?.collect();
        assert_eq!(vec![commits[1].oid()], commits[0].parents);

        assert_eq!(
            commits
                .iter()
                .map(|commit| commit.title_line().trim().to_owned())
                .collect::<Vec<_>>(),
            vec![
                String::from("Revert \"five\""),
                String::from("eight"),
                String::from("seven")
            ]
        );

        Ok(())
    }

    #[rstest]
    fn apply_multiple_non_conflicting_commits(mut helper: CommandHelper) -> Result<()> {
        helper
            .jit_cmd(&["revert", "@", "@^", "@^^"])
            .assert()
            .code(0);

        let revs = RevList::new(&helper.repo, &[String::from("@~4..")], Default::default())?;

        assert_eq!(
            revs.map(|commit| commit.title_line().trim().to_owned())
                .collect::<Vec<_>>(),
            vec![
                String::from("Revert \"six\""),
                String::from("Revert \"seven\""),
                String::from("Revert \"eight\""),
                String::from("eight")
            ]
        );

        let mut tree = HashMap::new();
        tree.insert("f.txt", "four");
        tree.insert("g.txt", "five");

        helper.assert_index(&tree)?;

        helper.assert_workspace(&tree)?;

        Ok(())
    }

    #[rstest]
    fn stop_when_a_list_of_commits_includes_a_conflict(mut helper: CommandHelper) {
        helper.jit_cmd(&["revert", "@^", "@"]).assert().code(1);

        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .stdout("UU g.txt\n");
    }

    #[rstest]
    fn stop_when_a_range_of_commits_includes_a_conflict(mut helper: CommandHelper) {
        helper.jit_cmd(&["revert", "@~5..@~2"]).assert().code(1);

        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .stdout("UD g.txt\n");
    }

    #[rstest]
    fn refuse_to_commit_in_a_conflicted_state(mut helper: CommandHelper) {
        helper.jit_cmd(&["revert", "@~5..@~2"]);

        helper.jit_cmd(&["commit"]).assert().code(128).stderr(
            "\
error: Committing is not possible because you have unmerged files.
hint: Fix them up in the work tree, and then use 'jit add/rm <file>'
hint: as appropriate to mark resolution and make a commit.
fatal: Exiting because of an unresolved conflict.
",
        );
    }

    #[rstest]
    fn refuse_to_continue_in_a_conflicted_state(mut helper: CommandHelper) {
        helper.jit_cmd(&["revert", "@~5..@~2"]);

        helper
            .jit_cmd(&["revert", "--continue"])
            .assert()
            .code(128)
            .stderr(
                "\
error: Committing is not possible because you have unmerged files.
hint: Fix them up in the work tree, and then use 'jit add/rm <file>'
hint: as appropriate to mark resolution and make a commit.
fatal: Exiting because of an unresolved conflict.
",
            );
    }

    #[rstest]
    fn can_continue_after_resolving_the_conflicts(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["revert", "@~4..@^"]);

        helper.write_file("g.txt", "five")?;
        helper.jit_cmd(&["add", "g.txt"]);

        helper.jit_cmd(&["revert", "--continue"]).assert().code(0);

        let revs = RevList::new(&helper.repo, &[String::from("@~4..")], Default::default())?;

        assert_eq!(
            revs.map(|commit| commit.title_line().trim().to_owned())
                .collect::<Vec<_>>(),
            vec![
                String::from("Revert \"five\""),
                String::from("Revert \"six\""),
                String::from("Revert \"seven\""),
                String::from("eight")
            ]
        );

        let mut tree = HashMap::new();
        tree.insert("f.txt", "four");

        helper.assert_index(&tree)?;
        helper.assert_workspace(&tree)?;

        Ok(())
    }

    #[rstest]
    fn can_continue_after_committing_the_resolved_tree(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["revert", "@~4..@^"]);

        helper.write_file("g.txt", "five")?;
        helper.jit_cmd(&["add", "g.txt"]);
        helper.jit_cmd(&["commit"]);

        helper.jit_cmd(&["revert", "--continue"]).assert().code(0);

        let revs = RevList::new(&helper.repo, &[String::from("@~4..")], Default::default())?;

        assert_eq!(
            revs.map(|commit| commit.title_line().trim().to_owned())
                .collect::<Vec<_>>(),
            vec![
                String::from("Revert \"five\""),
                String::from("Revert \"six\""),
                String::from("Revert \"seven\""),
                String::from("eight")
            ]
        );

        let mut tree = HashMap::new();
        tree.insert("f.txt", "four");

        helper.assert_index(&tree)?;
        helper.assert_workspace(&tree)?;

        Ok(())
    }

    #[rstest]
    fn aborting_in_a_conflicted_state(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["revert", "@~5..@^"]);
        helper
            .jit_cmd(&["revert", "--abort"])
            .assert()
            .code(0)
            .stderr("");

        // reset to the old HEAD
        assert_eq!(helper.load_commit("HEAD")?.message.trim(), "eight");

        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .stdout("");

        // remove the merge state
        assert!(!helper.repo.pending_commit().in_progress());

        Ok(())
    }

    #[rstest]
    fn aborting_in_a_committed_state(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["revert", "@~5..@^"]);
        helper.jit_cmd(&["add", "."]);
        helper.jit_cmd(&["commit"]);

        helper
            .jit_cmd(&["revert", "--abort"])
            .assert()
            .code(0)
            .stderr("warning: You seem to have moved HEAD. Not rewinding, check your HEAD!\n");

        // does not reset HEAD
        assert_eq!(
            helper.load_commit("HEAD")?.title_line().trim(),
            "Revert \"seven\""
        );

        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .stdout("");

        // remove the merge state
        assert!(!helper.repo.pending_commit().in_progress());

        Ok(())
    }
}
