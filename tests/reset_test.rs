mod common;

use assert_cmd::prelude::OutputAssertExt;
pub use common::CommandHelper;
use jit::errors::Result;
use rstest::{fixture, rstest};
use std::collections::HashMap;

mod with_no_head_commit {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.write_file("a.txt", "1").unwrap();
        helper.write_file("outer/b.txt", "2").unwrap();
        helper.write_file("outer/inner/c.txt", "3").unwrap();

        helper.jit_cmd(&["add", "."]);

        helper
    }

    fn assert_unchanged_workspace(helper: &CommandHelper) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("a.txt", "1");
        workspace.insert("outer/b.txt", "2");
        workspace.insert("outer/inner/c.txt", "3");
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn remove_everything_from_the_index(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["reset"]).assert().code(0);

        let index = HashMap::new();
        helper.assert_index(&index)?;
        assert_unchanged_workspace(&helper)?;

        Ok(())
    }

    #[rstest]
    fn remove_a_single_file_from_the_index(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["reset", "a.txt"]).assert().code(0);

        let mut index = HashMap::new();
        index.insert("outer/b.txt", "2");
        index.insert("outer/inner/c.txt", "3");
        helper.assert_index(&index)?;

        assert_unchanged_workspace(&helper)?;

        Ok(())
    }

    #[rstest]
    fn remove_a_directory_from_the_index(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["reset", "outer"]).assert().code(0);

        let mut index = HashMap::new();
        index.insert("a.txt", "1");
        helper.assert_index(&index)?;

        assert_unchanged_workspace(&helper)?;

        Ok(())
    }
}

mod with_a_head_commit {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.write_file("a.txt", "1").unwrap();
        helper.write_file("outer/b.txt", "2").unwrap();
        helper.write_file("outer/inner/c.txt", "3").unwrap();

        helper.jit_cmd(&["add", "."]);
        helper.commit("first");

        helper.write_file("outer/b.txt", "4").unwrap();
        helper.jit_cmd(&["add", "."]);
        helper.commit("second");

        helper.jit_cmd(&["rm", "a.txt"]);
        helper.write_file("outer/d.txt", "5").unwrap();
        helper.write_file("outer/inner/c.txt", "6").unwrap();
        helper.jit_cmd(&["add", "."]);
        helper.write_file("outer/e.txt", "7").unwrap();

        helper.head_oid = helper.repo.refs.read_head().unwrap();

        helper
    }

    fn assert_unchanged_head(helper: &CommandHelper) -> Result<()> {
        assert_eq!(helper.repo.refs.read_head()?, helper.head_oid);

        Ok(())
    }

    fn assert_unchanged_workspace(helper: &CommandHelper) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("outer/b.txt", "4");
        workspace.insert("outer/d.txt", "5");
        workspace.insert("outer/e.txt", "7");
        workspace.insert("outer/inner/c.txt", "6");
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn restore_a_file_removed_from_the_index(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["reset", "a.txt"]).assert().code(0);

        let mut index = HashMap::new();
        index.insert("a.txt", "1");
        index.insert("outer/b.txt", "4");
        index.insert("outer/d.txt", "5");
        index.insert("outer/inner/c.txt", "6");
        helper.assert_index(&index)?;

        assert_unchanged_head(&helper)?;
        assert_unchanged_workspace(&helper)?;

        Ok(())
    }

    #[rstest]
    fn reset_a_file_modified_in_the_index(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["reset", "outer/inner"]).assert().code(0);

        let mut index = HashMap::new();
        index.insert("outer/b.txt", "4");
        index.insert("outer/d.txt", "5");
        index.insert("outer/inner/c.txt", "3");
        helper.assert_index(&index)?;

        assert_unchanged_head(&helper)?;
        assert_unchanged_workspace(&helper)?;

        Ok(())
    }

    #[rstest]
    fn remove_a_file_added_to_the_index(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["reset", "outer/d.txt"]).assert().code(0);

        let mut index = HashMap::new();
        index.insert("outer/b.txt", "4");
        index.insert("outer/inner/c.txt", "6");
        helper.assert_index(&index)?;

        assert_unchanged_head(&helper)?;
        assert_unchanged_workspace(&helper)?;

        Ok(())
    }

    #[rstest]
    fn reset_a_file_to_a_specific_commit(mut helper: CommandHelper) -> Result<()> {
        helper
            .jit_cmd(&["reset", "@^", "outer/b.txt"])
            .assert()
            .code(0);

        let mut index = HashMap::new();
        index.insert("outer/b.txt", "2");
        index.insert("outer/d.txt", "5");
        index.insert("outer/inner/c.txt", "6");
        helper.assert_index(&index)?;

        assert_unchanged_head(&helper)?;
        assert_unchanged_workspace(&helper)?;

        Ok(())
    }

    #[rstest]
    fn reset_the_whole_index(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["reset"]).assert().code(0);

        let mut index = HashMap::new();
        index.insert("a.txt", "1");
        index.insert("outer/b.txt", "4");
        index.insert("outer/inner/c.txt", "3");
        helper.assert_index(&index)?;

        assert_unchanged_head(&helper)?;
        assert_unchanged_workspace(&helper)?;

        Ok(())
    }

    #[rstest]
    fn reset_the_whole_index_and_move_head(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["reset", "@^"]).assert().code(0);

        let mut index = HashMap::new();
        index.insert("a.txt", "1");
        index.insert("outer/b.txt", "2");
        index.insert("outer/inner/c.txt", "3");
        helper.assert_index(&index)?;

        assert_eq!(
            helper.repo.refs.read_head()?,
            helper
                .repo
                .database
                .load_commit(helper.head_oid.as_ref().unwrap())?
                .parent()
        );

        assert_unchanged_workspace(&helper)?;

        Ok(())
    }

    #[rstest]
    fn move_head_and_leave_the_index_unchanged(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["reset", "--soft", "@^"]).assert().code(0);

        let mut index = HashMap::new();
        index.insert("outer/b.txt", "4");
        index.insert("outer/d.txt", "5");
        index.insert("outer/inner/c.txt", "6");
        helper.assert_index(&index)?;

        assert_eq!(
            helper.repo.refs.read_head()?,
            helper
                .repo
                .database
                .load_commit(helper.head_oid.as_ref().unwrap())?
                .parent()
        );

        assert_unchanged_workspace(&helper)?;

        Ok(())
    }

    #[rstest]
    fn reset_the_index_and_workspace(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("a.txt/nested", "remove me")?;
        helper.write_file("outer/b.txt", "10")?;
        helper.delete("outer/inner")?;

        helper.jit_cmd(&["reset", "--hard"]).assert().code(0);
        assert_unchanged_head(&helper)?;

        let mut index = HashMap::new();
        index.insert("a.txt", "1");
        index.insert("outer/b.txt", "4");
        index.insert("outer/inner/c.txt", "3");
        helper.assert_index(&index)?;

        helper
            .jit_cmd(&["status", "--porcelain"])
            .assert()
            .stdout("?? outer/e.txt\n");

        Ok(())
    }

    #[rstest]
    fn let_you_return_to_the_previous_state_using_orig_head(
        mut helper: CommandHelper,
    ) -> Result<()> {
        helper.jit_cmd(&["reset", "--hard", "@^"]).assert().code(0);

        let mut index = HashMap::new();
        index.insert("a.txt", "1");
        index.insert("outer/b.txt", "2");
        index.insert("outer/inner/c.txt", "3");
        helper.assert_index(&index)?;

        helper
            .jit_cmd(&["reset", "--hard", "ORIG_HEAD"])
            .assert()
            .code(0);

        let mut index = HashMap::new();
        index.insert("a.txt", "1");
        index.insert("outer/b.txt", "4");
        index.insert("outer/inner/c.txt", "3");
        helper.assert_index(&index)?;

        Ok(())
    }
}
