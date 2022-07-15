mod common;

use std::collections::HashMap;
use std::path::PathBuf;

use assert_cmd::prelude::OutputAssertExt;
pub use common::CommandHelper;
use jit::errors::Result;
use rstest::{fixture, rstest};

mod with_a_single_file {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.write_file("f.txt", "1").unwrap();

        helper.jit_cmd(&["add", "."]);
        helper.commit("first");

        helper
    }

    #[rstest]
    fn exit_successfully(mut helper: CommandHelper) {
        helper.jit_cmd(&["rm", "f.txt"]).assert().code(0);
    }

    #[rstest]
    fn remove_a_file_from_the_index(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["rm", "f.txt"]);

        helper.repo.index.load()?;
        assert!(!helper.repo.index.tracked_file(&PathBuf::from("f.txt")));

        Ok(())
    }

    #[rstest]
    fn remove_a_file_from_the_workspace(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["rm", "f.txt"]);

        let workspace = HashMap::new();
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn succeed_if_the_file_is_not_in_the_workspace(mut helper: CommandHelper) -> Result<()> {
        helper.delete("f.txt")?;
        helper.jit_cmd(&["rm", "f.txt"]).assert().code(0);

        helper.repo.index.load()?;
        assert!(!helper.repo.index.tracked_file(&PathBuf::from("f.txt")));

        Ok(())
    }

    #[rstest]
    fn fail_if_the_file_has_unstaged_changed(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("f.txt", "2")?;

        helper.jit_cmd(&["rm", "f.txt"]).assert().code(1).stderr(
            "\
error: the following file has local modifications:
    f.txt
",
        );

        helper.repo.index.load()?;
        assert!(helper.repo.index.tracked_file(&PathBuf::from("f.txt")));

        let workspace = HashMap::from([("f.txt", "2")]);
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn fail_if_the_file_is_not_in_the_index(mut helper: CommandHelper) -> Result<()> {
        helper
            .jit_cmd(&["rm", "nope.txt"])
            .assert()
            .code(128)
            .stderr("fatal: pathspec 'nope.txt' did not match any files\n");

        Ok(())
    }

    #[rstest]
    fn fail_if_the_file_has_uncommitted_changes(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("f.txt", "2")?;
        helper.jit_cmd(&["add", "f.txt"]);

        helper.jit_cmd(&["rm", "f.txt"]).assert().code(1).stderr(
            "\
error: the following file has changes staged in the index:
    f.txt
",
        );

        helper.repo.index.load()?;
        assert!(helper.repo.index.tracked_file(&PathBuf::from("f.txt")));

        let workspace = HashMap::from([("f.txt", "2")]);
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn force_removal_of_unstaged_changes(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("f.txt", "2")?;
        helper.jit_cmd(&["rm", "-f", "f.txt"]);

        helper.repo.index.load()?;
        assert!(!helper.repo.index.tracked_file(&PathBuf::from("f.txt")));

        let workspace = HashMap::new();
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn force_removal_of_uncommitted_changes(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("f.txt", "2")?;
        helper.jit_cmd(&["add", "f.txt"]);
        helper.jit_cmd(&["rm", "-f", "f.txt"]);

        helper.repo.index.load()?;
        assert!(!helper.repo.index.tracked_file(&PathBuf::from("f.txt")));

        let workspace = HashMap::new();
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn remove_a_file_only_from_the_index(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["rm", "--cached", "f.txt"]);

        helper.repo.index.load()?;
        assert!(!helper.repo.index.tracked_file(&PathBuf::from("f.txt")));

        let workspace = HashMap::from([("f.txt", "1")]);
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn remove_a_file_from_the_index_if_it_has_unstaged_changes(
        mut helper: CommandHelper,
    ) -> Result<()> {
        helper.write_file("f.txt", "2")?;
        helper.jit_cmd(&["rm", "--cached", "f.txt"]);

        helper.repo.index.load()?;
        assert!(!helper.repo.index.tracked_file(&PathBuf::from("f.txt")));

        let workspace = HashMap::from([("f.txt", "2")]);
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn remove_a_file_from_the_index_if_it_has_uncommitted_changes(
        mut helper: CommandHelper,
    ) -> Result<()> {
        helper.write_file("f.txt", "2")?;
        helper.jit_cmd(&["add", "f.txt"]);
        helper.jit_cmd(&["rm", "--cached", "f.txt"]);

        helper.repo.index.load()?;
        assert!(!helper.repo.index.tracked_file(&PathBuf::from("f.txt")));

        let workspace = HashMap::from([("f.txt", "2")]);
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn do_not_remove_a_file_with_both_uncommitted_and_unstaged_changes(
        mut helper: CommandHelper,
    ) -> Result<()> {
        helper.write_file("f.txt", "2")?;
        helper.jit_cmd(&["add", "f.txt"]);
        helper.write_file("f.txt", "3")?;
        helper
            .jit_cmd(&["rm", "--cached", "f.txt"])
            .assert()
            .code(1)
            .stderr(
                "\
error: the following file has staged content different from both the file and the HEAD:
    f.txt
",
            );

        helper.repo.index.load()?;
        assert!(helper.repo.index.tracked_file(&PathBuf::from("f.txt")));

        let workspace = HashMap::from([("f.txt", "3")]);
        helper.assert_workspace(&workspace)?;

        Ok(())
    }
}

mod with_no_commit {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.write_file("f.txt", "1").unwrap();

        helper
    }

    #[rstest]
    fn fail_if_the_file_has_uncommitted_changes(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["add", "f.txt"]);

        helper.jit_cmd(&["rm", "f.txt"]).assert().code(1).stderr(
            "\
error: the following file has changes staged in the index:
    f.txt
",
        );

        let workspace = HashMap::from([("f.txt", "1")]);
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn fail_if_the_file_is_not_in_the_index(mut helper: CommandHelper) -> Result<()> {
        helper
            .jit_cmd(&["rm", "f.txt"])
            .assert()
            .code(128)
            .stderr("fatal: pathspec 'f.txt' did not match any files\n");

        Ok(())
    }
}

mod with_a_tree {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.write_file("f.txt", "1").unwrap();
        helper.write_file("outer/g.txt", "2").unwrap();
        helper.write_file("outer/inner/h.txt", "3").unwrap();

        helper.jit_cmd(&["add", "."]);
        helper.commit("first");

        helper
    }

    #[rstest]
    fn remove_multiple_files(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["rm", "f.txt", "outer/inner/h.txt"]);

        helper.repo.index.load()?;
        assert_eq!(
            helper
                .repo
                .index
                .entries
                .values()
                .map(|entry| entry.path.clone())
                .collect::<Vec<_>>(),
            vec!["outer/g.txt"]
        );

        let workspace = HashMap::from([("outer/g.txt", "2")]);
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn refuse_to_remove_a_directory(mut helper: CommandHelper) -> Result<()> {
        helper
            .jit_cmd(&["rm", "f.txt", "outer"])
            .assert()
            .code(128)
            .stderr("fatal: not removing 'outer' recursively without -r\n");

        helper.repo.index.load()?;
        assert_eq!(
            helper
                .repo
                .index
                .entries
                .values()
                .map(|entry| entry.path.clone())
                .collect::<Vec<_>>(),
            vec!["f.txt", "outer/g.txt", "outer/inner/h.txt"]
        );

        let workspace = HashMap::from([
            ("f.txt", "1"),
            ("outer/g.txt", "2"),
            ("outer/inner/h.txt", "3"),
        ]);
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn do_not_remove_a_file_replaced_with_a_directory(mut helper: CommandHelper) -> Result<()> {
        helper.delete("f.txt")?;
        helper.write_file("f.txt/nested", "keep me")?;

        helper
            .jit_cmd(&["rm", "f.txt"])
            .assert()
            .code(128)
            .stderr("fatal: jit rm: 'f.txt': Operation not permitted\n");

        helper.repo.index.load()?;
        assert_eq!(
            helper
                .repo
                .index
                .entries
                .values()
                .map(|entry| entry.path.clone())
                .collect::<Vec<_>>(),
            vec!["f.txt", "outer/g.txt", "outer/inner/h.txt"]
        );

        let workspace = HashMap::from([
            ("f.txt/nested", "keep me"),
            ("outer/g.txt", "2"),
            ("outer/inner/h.txt", "3"),
        ]);
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn remove_a_directory_with_dash_r(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["rm", "-r", "outer"]);

        helper.repo.index.load()?;
        assert_eq!(
            helper
                .repo
                .index
                .entries
                .values()
                .map(|entry| entry.path.clone())
                .collect::<Vec<_>>(),
            vec!["f.txt"]
        );

        let workspace = HashMap::from([("f.txt", "1")]);
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn do_not_remove_untracked_files(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("outer/inner/j.txt", "4")?;
        helper.jit_cmd(&["rm", "-r", "outer"]);

        helper.repo.index.load()?;
        assert_eq!(
            helper
                .repo
                .index
                .entries
                .values()
                .map(|entry| entry.path.clone())
                .collect::<Vec<_>>(),
            vec!["f.txt"]
        );

        let workspace = HashMap::from([("f.txt", "1"), ("outer/inner/j.txt", "4")]);
        helper.assert_workspace(&workspace)?;

        Ok(())
    }
}
