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
}
