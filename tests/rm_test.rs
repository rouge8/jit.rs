mod common;

use assert_cmd::prelude::OutputAssertExt;
pub use common::CommandHelper;
use jit::errors::Result;
use rstest::{fixture, rstest};
use std::collections::HashMap;
use std::path::PathBuf;

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

        let mut repo = helper.repo();
        repo.index.load()?;
        assert!(!repo.index.tracked_file(&PathBuf::from("f.txt")));

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

        let mut repo = helper.repo();
        repo.index.load()?;
        assert!(!repo.index.tracked_file(&PathBuf::from("f.txt")));

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

        let mut repo = helper.repo();
        repo.index.load()?;
        assert!(repo.index.tracked_file(&PathBuf::from("f.txt")));

        let mut workspace = HashMap::new();
        workspace.insert("f.txt", "2");
        helper.assert_workspace(&workspace)?;

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

        let mut repo = helper.repo();
        repo.index.load()?;
        assert!(repo.index.tracked_file(&PathBuf::from("f.txt")));

        let mut workspace = HashMap::new();
        workspace.insert("f.txt", "2");
        helper.assert_workspace(&workspace)?;

        Ok(())
    }
}
