mod common;

use assert_cmd::prelude::OutputAssertExt;
pub use common::CommandHelper;
use jit::errors::Result;
use lazy_static::lazy_static;
use rstest::{fixture, rstest};
use std::collections::HashMap;

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
}
