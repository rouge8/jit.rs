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
            let mut tree = HashMap::new();
            tree.insert("f.txt", message);
            commit_tree(&mut helper, message, &tree).unwrap();
        }

        helper.jit_cmd(&["branch", "topic", "@~2"]);
        helper.jit_cmd(&["checkout", "topic"]);

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

        helper.jit_cmd(&["checkout", "main"]);

        helper
    }

    #[rstest]
    fn apply_a_commit_on_top_of_the_current_head(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["cherry-pick", "topic~3"]).assert().code(0);

        let revs = RevList::new(&helper.repo, &[String::from("@~3..")])?;

        assert_eq!(
            revs.map(|commit| commit.message.trim().to_owned())
                .collect::<Vec<_>>(),
            vec![
                String::from("five"),
                String::from("four"),
                String::from("three")
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

        let mut workspace = HashMap::new();
        workspace.insert("f.txt", conflict);
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

        let mut workspace = HashMap::new();
        workspace.insert("f.txt", "four");
        workspace.insert("g.txt", "eight");
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

        let commits: Vec<_> = RevList::new(&helper.repo, &[String::from("@~3..")])?.collect();
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

        let mut tree = HashMap::new();
        tree.insert("f.txt", "four");
        tree.insert("g.txt", "eight");

        helper.assert_index(&tree)?;

        helper.assert_workspace(&tree)?;

        Ok(())
    }

    #[rstest]
    fn commit_after_a_conflicted_cherry_pick(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["cherry-pick", "topic"]);
        helper.jit_cmd(&["add", "g.txt"]);

        helper.jit_cmd(&["commit"]).assert().code(0);

        let commits: Vec<_> = RevList::new(&helper.repo, &[String::from("@~3..")])?.collect();
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
}
