mod common;

pub use common::{helper, CommandHelper};
use jit::errors::Result;
use jit::repository::Repository;
use rstest::{fixture, rstest};

#[rstest]
fn list_untracked_files_in_name_order(mut helper: CommandHelper) -> Result<()> {
    helper.write_file("file.txt", "")?;
    helper.write_file("another.txt", "")?;

    helper.assert_status(
        "\
?? another.txt
?? file.txt
",
    );

    Ok(())
}

#[rstest]
fn list_files_as_untracked_if_they_are_not_in_the_index(mut helper: CommandHelper) -> Result<()> {
    helper.write_file("committed.txt", "")?;
    helper.jit_cmd(&["add", "."]);
    helper.commit("commit message");

    helper.write_file("file.txt", "")?;

    helper.assert_status("?? file.txt\n");

    Ok(())
}

#[rstest]
fn list_untracked_directories_not_their_contents(mut helper: CommandHelper) -> Result<()> {
    helper.write_file("file.txt", "")?;
    helper.write_file("dir/another.txt", "")?;

    helper.assert_status(
        "\
?? dir/
?? file.txt
",
    );

    Ok(())
}

#[rstest]
fn list_untracked_files_inside_tracked_directories(mut helper: CommandHelper) -> Result<()> {
    helper.write_file("a/b/inner.txt", "")?;
    helper.jit_cmd(&["add", "."]);
    helper.commit("commit message");

    helper.write_file("a/outer.txt", "")?;
    helper.write_file("a/b/c/file.txt", "")?;

    helper.assert_status(
        "\
?? a/b/c/
?? a/outer.txt
",
    );

    Ok(())
}

#[rstest]
fn dont_list_empty_untracked_directories(mut helper: CommandHelper) -> Result<()> {
    helper.mkdir("outer")?;

    helper.assert_status("");

    Ok(())
}

#[rstest]
fn list_untracked_directories_that_indirectly_contain_files(
    mut helper: CommandHelper,
) -> Result<()> {
    helper.write_file("outer/inner/file.txt", "")?;

    helper.assert_status("?? outer/\n");

    Ok(())
}

mod index_workspace_changes {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.write_file("1.txt", "one").unwrap();
        helper.write_file("a/2.txt", "two").unwrap();
        helper.write_file("a/b/3.txt", "three").unwrap();
        helper.jit_cmd(&["add", "."]);
        helper.commit("commit message");

        helper
    }

    #[rstest]
    fn print_nothing_when_no_files_are_changed(mut helper: CommandHelper) -> Result<()> {
        helper.assert_status("");

        Ok(())
    }

    #[rstest]
    fn report_files_with_modified_contents(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("1.txt", "changed")?;
        helper.write_file("a/2.txt", "modified")?;

        helper.assert_status(
            " M 1.txt
 M a/2.txt
",
        );

        Ok(())
    }

    #[rstest]
    fn report_files_with_changed_modes(mut helper: CommandHelper) -> Result<()> {
        helper.make_executable("a/2.txt")?;

        helper.assert_status(" M a/2.txt\n");

        Ok(())
    }

    #[rstest]
    fn report_modified_files_with_unchanged_size(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("a/b/3.txt", "hello")?;

        helper.assert_status(" M a/b/3.txt\n");

        Ok(())
    }

    #[rstest]
    fn print_nothing_if_a_file_is_touched(mut helper: CommandHelper) -> Result<()> {
        let mut index = Repository::new(helper.repo_path.join(".git")).index;
        index.load()?;
        let entry_before = &index.entries[&(String::from("1.txt"), 0)];

        helper.touch("1.txt")?;

        helper.assert_status("");

        let mut index = Repository::new(helper.repo_path.join(".git")).index;
        index.load()?;
        let entry_after = &index.entries[&(String::from("1.txt"), 0)];

        // The modification time should have been updated in the index
        assert_ne!(
            (entry_before.mtime, entry_before.mtime_nsec),
            (entry_after.mtime, entry_after.mtime_nsec)
        );

        Ok(())
    }

    #[rstest]
    fn report_deleted_files(mut helper: CommandHelper) -> Result<()> {
        helper.delete("a/2.txt")?;

        helper.assert_status(" D a/2.txt\n");

        Ok(())
    }

    #[rstest]
    fn report_files_in_deleted_directories(mut helper: CommandHelper) -> Result<()> {
        helper.delete("a")?;

        helper.assert_status(
            " D a/2.txt
 D a/b/3.txt
",
        );

        Ok(())
    }
}

mod head_index_changes {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.write_file("1.txt", "one").unwrap();
        helper.write_file("a/2.txt", "two").unwrap();
        helper.write_file("a/b/3.txt", "three").unwrap();
        helper.jit_cmd(&["add", "."]);
        helper.commit("commit message");

        helper
    }

    #[rstest]
    fn report_a_file_added_to_a_tracked_directory(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("a/4.txt", "four")?;
        helper.jit_cmd(&["add", "."]);

        helper.assert_status("A  a/4.txt\n");

        Ok(())
    }

    #[rstest]
    fn report_a_file_added_to_an_untracked_directory(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("d/e/5.txt", "five")?;
        helper.jit_cmd(&["add", "."]);

        helper.assert_status("A  d/e/5.txt\n");

        Ok(())
    }

    #[rstest]
    fn report_modified_modes(mut helper: CommandHelper) -> Result<()> {
        helper.make_executable("1.txt")?;
        helper.jit_cmd(&["add", "."]);

        helper.assert_status("M  1.txt\n");

        Ok(())
    }

    #[rstest]
    fn report_modified_contents(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("a/b/3.txt", "changed")?;
        helper.jit_cmd(&["add", "."]);

        helper.assert_status("M  a/b/3.txt\n");

        Ok(())
    }

    #[rstest]
    fn report_deleted_files(mut helper: CommandHelper) -> Result<()> {
        helper.delete("1.txt")?;
        helper.delete(".git/index")?;
        helper.jit_cmd(&["add", "."]);

        helper.assert_status("D  1.txt\n");

        Ok(())
    }

    #[rstest]
    fn report_all_deleted_files_inside_directories(mut helper: CommandHelper) -> Result<()> {
        helper.delete("a")?;
        helper.delete(".git/index")?;
        helper.jit_cmd(&["add", "."]);

        helper.assert_status(
            "\
D  a/2.txt
D  a/b/3.txt
",
        );

        Ok(())
    }
}
