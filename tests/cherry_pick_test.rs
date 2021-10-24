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

mod with_two_branches {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        for message in ["one", "two", "three", "four"] {
            let tree = HashMap::from([("f.txt", message)]);
            commit_tree(&mut helper, message, &tree).unwrap();
        }

        helper.jit_cmd(&["branch", "topic", "@~2"]);
        helper.jit_cmd(&["checkout", "topic"]);

        let tree = HashMap::from([("g.txt", "five")]);
        commit_tree(&mut helper, "five", &tree).unwrap();

        let tree = HashMap::from([("f.txt", "six")]);
        commit_tree(&mut helper, "six", &tree).unwrap();

        let tree = HashMap::from([("g.txt", "seven")]);
        commit_tree(&mut helper, "seven", &tree).unwrap();

        let tree = HashMap::from([("g.txt", "eight")]);
        commit_tree(&mut helper, "eight", &tree).unwrap();

        helper.jit_cmd(&["checkout", "main"]);

        helper
    }

    #[rstest]
    fn apply_a_commit_on_top_of_the_current_head(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["cherry-pick", "topic~3"]).assert().code(0);

        let revs = RevList::new(&helper.repo, &[String::from("@~3..")], Default::default())?;

        assert_eq!(
            revs.map(|commit| commit.message.trim().to_owned())
                .collect::<Vec<_>>(),
            vec![
                String::from("five"),
                String::from("four"),
                String::from("three")
            ]
        );

        let tree = HashMap::from([("f.txt", "four"), ("g.txt", "five")]);

        helper.assert_index(&tree)?;

        helper.assert_workspace(&tree)?;

        Ok(())
    }

    #[rstest]
    fn fail_to_apply_a_content_conflict(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["cherry-pick", "topic^^"]).assert().code(1);

        let short = Database::short_oid(&helper.resolve_revision("topic^^")?);

        let conflict = format!(
            "\
<<<<<<< HEAD
four=======
six>>>>>>> {}... six
",
            short
        );
        let conflict = conflict.as_str();

        let workspace = HashMap::from([("f.txt", conflict)]);
        helper.assert_workspace(&workspace)?;

        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .stdout("UU f.txt\n");

        Ok(())
    }

    #[rstest]
    fn fail_to_apply_a_modify_delete_conflict(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["cherry-pick", "topic"]).assert().code(1);

        let workspace = HashMap::from([("f.txt", "four"), ("g.txt", "eight")]);
        helper.assert_workspace(&workspace)?;

        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .stdout("DU g.txt\n");

        Ok(())
    }

    #[rstest]
    fn continue_a_conflicted_cherry_pick(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["cherry-pick", "topic"]);
        helper.jit_cmd(&["add", "g.txt"]);

        helper
            .jit_cmd(&["cherry-pick", "--continue"])
            .assert()
            .code(0);

        let commits: Vec<_> =
            RevList::new(&helper.repo, &[String::from("@~3..")], Default::default())?.collect();
        assert_eq!(commits[0].parents, vec![commits[1].oid()]);

        assert_eq!(
            commits
                .iter()
                .map(|commit| commit.message.trim().to_owned())
                .collect::<Vec<_>>(),
            vec![
                String::from("eight"),
                String::from("four"),
                String::from("three")
            ]
        );

        let tree = HashMap::from([("f.txt", "four"), ("g.txt", "eight")]);

        helper.assert_index(&tree)?;

        helper.assert_workspace(&tree)?;

        Ok(())
    }

    #[rstest]
    fn commit_after_a_conflicted_cherry_pick(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["cherry-pick", "topic"]);
        helper.jit_cmd(&["add", "g.txt"]);

        helper.jit_cmd(&["commit"]).assert().code(0);

        let commits: Vec<_> =
            RevList::new(&helper.repo, &[String::from("@~3..")], Default::default())?.collect();
        assert_eq!(commits[0].parents, vec![commits[1].oid()]);

        assert_eq!(
            commits
                .iter()
                .map(|commit| commit.message.trim().to_owned())
                .collect::<Vec<_>>(),
            vec![
                String::from("eight"),
                String::from("four"),
                String::from("three")
            ]
        );

        Ok(())
    }

    #[rstest]
    fn apply_multiple_non_conflicting_commits(mut helper: CommandHelper) -> Result<()> {
        helper
            .jit_cmd(&["cherry-pick", "topic~3", "topic^", "topic"])
            .assert()
            .code(0);

        let revs = RevList::new(&helper.repo, &[String::from("@~4..")], Default::default())?;

        assert_eq!(
            revs.map(|commit| commit.message.trim().to_owned())
                .collect::<Vec<_>>(),
            vec![
                String::from("eight"),
                String::from("seven"),
                String::from("five"),
                String::from("four")
            ]
        );

        let tree = HashMap::from([("f.txt", "four"), ("g.txt", "eight")]);

        helper.assert_index(&tree)?;

        helper.assert_workspace(&tree)?;

        Ok(())
    }

    #[rstest]
    fn stop_when_a_list_of_commits_includes_a_conflict(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["cherry-pick", "topic^", "topic~3"])
            .assert()
            .code(1);

        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .stdout("DU g.txt\n");
    }

    #[rstest]
    fn stop_when_a_range_of_commits_includes_a_conflict(mut helper: CommandHelper) {
        helper.jit_cmd(&["cherry-pick", "..topic"]).assert().code(1);

        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .stdout("UU f.txt\n");
    }

    #[rstest]
    fn refuse_to_commit_in_a_conflicted_state(mut helper: CommandHelper) {
        helper.jit_cmd(&["cherry-pick", "..topic"]).assert().code(1);

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
        helper.jit_cmd(&["cherry-pick", "..topic"]).assert().code(1);

        helper
            .jit_cmd(&["cherry-pick", "--continue"])
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
        helper.jit_cmd(&["cherry-pick", "..topic"]);

        helper.write_file("f.txt", "six")?;
        helper.jit_cmd(&["add", "f.txt"]);

        helper
            .jit_cmd(&["cherry-pick", "--continue"])
            .assert()
            .code(0);

        let revs = RevList::new(&helper.repo, &[String::from("@~5..")], Default::default())?;

        assert_eq!(
            revs.map(|commit| commit.message.trim().to_owned())
                .collect::<Vec<_>>(),
            vec![
                String::from("eight"),
                String::from("seven"),
                String::from("six"),
                String::from("five"),
                String::from("four")
            ]
        );

        let tree = HashMap::from([("f.txt", "six"), ("g.txt", "eight")]);

        helper.assert_index(&tree)?;

        helper.assert_workspace(&tree)?;

        Ok(())
    }

    #[rstest]
    fn can_continue_after_commiting_the_resolved_tree(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["cherry-pick", "..topic"]);

        helper.write_file("f.txt", "six")?;
        helper.jit_cmd(&["add", "f.txt"]);
        helper.jit_cmd(&["commit"]);

        helper
            .jit_cmd(&["cherry-pick", "--continue"])
            .assert()
            .code(0);

        let revs = RevList::new(&helper.repo, &[String::from("@~5..")], Default::default())?;

        assert_eq!(
            revs.map(|commit| commit.message.trim().to_owned())
                .collect::<Vec<_>>(),
            vec![
                String::from("eight"),
                String::from("seven"),
                String::from("six"),
                String::from("five"),
                String::from("four")
            ]
        );

        let tree = HashMap::from([("f.txt", "six"), ("g.txt", "eight")]);

        helper.assert_index(&tree)?;

        helper.assert_workspace(&tree)?;

        Ok(())
    }

    #[rstest]
    fn aborting_in_a_conflicted_state(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["cherry-pick", "..topic"]);
        helper
            .jit_cmd(&["cherry-pick", "--abort"])
            .assert()
            .code(0)
            .stderr("");

        // reset to the old HEAD
        assert_eq!(helper.load_commit("HEAD")?.message.trim(), "four");

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
        helper.jit_cmd(&["cherry-pick", "..topic"]);
        helper.jit_cmd(&["add", "."]);
        helper.jit_cmd(&["commit"]);

        helper
            .jit_cmd(&["cherry-pick", "--abort"])
            .assert()
            .code(0)
            .stderr("warning: You seem to have moved HEAD. Not rewinding, check your HEAD!\n");

        // don't reset HEAD
        assert_eq!(helper.load_commit("HEAD")?.message.trim(), "six");

        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .stdout("");

        // remove the merge state
        assert!(!helper.repo.pending_commit().in_progress());

        Ok(())
    }
}

///   f---f---f---f [main]
///        \
///         g---h---o---o [topic]
///          \     /   /
///           j---j---f [side]
mod with_merges {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        // Commit to `main`
        for message in ["one", "two", "three", "four"] {
            let tree = HashMap::from([("f.txt", message)]);
            commit_tree(&mut helper, message, &tree).unwrap();
        }

        // Commit to `topic`
        helper.jit_cmd(&["branch", "topic", "@~2"]);
        helper.jit_cmd(&["checkout", "topic"]);

        let tree = HashMap::from([("g.txt", "five")]);
        commit_tree(&mut helper, "five", &tree).unwrap();

        let tree = HashMap::from([("h.txt", "six")]);
        commit_tree(&mut helper, "six", &tree).unwrap();

        // Commit to `side`
        helper.jit_cmd(&["branch", "side", "@^"]);
        helper.jit_cmd(&["checkout", "side"]);

        let tree = HashMap::from([("j.txt", "seven")]);
        commit_tree(&mut helper, "seven", &tree).unwrap();

        let tree = HashMap::from([("j.txt", "eight")]);
        commit_tree(&mut helper, "eight", &tree).unwrap();

        let tree = HashMap::from([("f.txt", "nine")]);
        commit_tree(&mut helper, "nine", &tree).unwrap();

        // Merge `side` into `topic`
        helper.jit_cmd(&["checkout", "topic"]);
        helper.jit_cmd(&["merge", "side^", "-m", "merge side^"]);
        helper.jit_cmd(&["merge", "side", "-m", "merge side"]);

        // Back to `main`
        helper.jit_cmd(&["checkout", "main"]);

        helper
    }

    #[rstest]
    fn refuse_to_cherry_pick_a_merge_without_specifying_a_parent(
        mut helper: CommandHelper,
    ) -> Result<()> {
        let oid = helper.resolve_revision("topic")?;

        helper
            .jit_cmd(&["cherry-pick", "topic"])
            .assert()
            .code(1)
            .stderr(format!(
                "error: commit {} is a merge but no -m option was given\n",
                oid
            ));

        Ok(())
    }

    #[rstest]
    fn refuse_to_cherry_pick_a_non_merge_commit_with_mainline(
        mut helper: CommandHelper,
    ) -> Result<()> {
        let oid = helper.resolve_revision("side")?;

        helper
            .jit_cmd(&["cherry-pick", "-m", "1", "side"])
            .assert()
            .code(1)
            .stderr(format!(
                "error: mainline was specified but commit {} is not a merge\n",
                oid
            ));

        Ok(())
    }

    #[rstest]
    fn cherry_pick_a_merge_based_on_its_first_parent(mut helper: CommandHelper) -> Result<()> {
        helper
            .jit_cmd(&["cherry-pick", "-m", "1", "topic^"])
            .assert()
            .code(0);

        let tree = HashMap::from([("f.txt", "four"), ("j.txt", "eight")]);

        helper.assert_index(&tree)?;

        helper.assert_workspace(&tree)?;

        Ok(())
    }

    #[rstest]
    fn cherry_pick_a_merge_based_on_its_second_parent(mut helper: CommandHelper) -> Result<()> {
        helper
            .jit_cmd(&["cherry-pick", "-m", "2", "topic^"])
            .assert()
            .code(0);

        let tree = HashMap::from([("f.txt", "four"), ("h.txt", "six")]);

        helper.assert_index(&tree)?;

        helper.assert_workspace(&tree)?;

        Ok(())
    }

    #[rstest]
    fn resume_cherry_picking_merges_after_a_conflict(mut helper: CommandHelper) -> Result<()> {
        helper
            .jit_cmd(&["cherry-pick", "-m", "1", "topic", "topic^"])
            .assert()
            .code(1);

        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .stdout("UU f.txt\n");

        helper.write_file("f.txt", "resolved")?;
        helper.jit_cmd(&["add", "f.txt"]);
        helper
            .jit_cmd(&["cherry-pick", "--continue"])
            .assert()
            .code(0);

        let revs = RevList::new(&helper.repo, &[String::from("@~3..")], Default::default())?;

        assert_eq!(
            revs.map(|commit| commit.message.trim().to_owned())
                .collect::<Vec<_>>(),
            vec![
                String::from("merge side^"),
                String::from("merge side"),
                String::from("four")
            ]
        );

        let tree = HashMap::from([("f.txt", "resolved"), ("j.txt", "eight")]);

        helper.assert_index(&tree)?;

        helper.assert_workspace(&tree)?;

        Ok(())
    }
}
