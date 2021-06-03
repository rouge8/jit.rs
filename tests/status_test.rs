mod common;

pub use common::CommandHelper;
use jit::errors::Result;

#[test]
fn list_untracked_files_in_name_order() -> Result<()> {
    let mut helper = CommandHelper::new();
    helper.init();

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

#[test]
fn list_files_as_untracked_if_they_are_not_in_the_index() -> Result<()> {
    let mut helper = CommandHelper::new();
    helper.init();

    helper.write_file("committed.txt", "")?;
    helper.jit_cmd(&["add", "."]);
    helper.commit("commit message");

    helper.write_file("file.txt", "")?;

    helper.assert_status("?? file.txt\n");

    Ok(())
}

#[test]
fn list_untracked_directories_not_their_contents() -> Result<()> {
    let mut helper = CommandHelper::new();
    helper.init();

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

#[test]
fn list_untracked_files_inside_tracked_directories() -> Result<()> {
    let mut helper = CommandHelper::new();
    helper.init();

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

#[test]
fn dont_list_empty_untracked_directories() -> Result<()> {
    let mut helper = CommandHelper::new();
    helper.init();

    helper.mkdir("outer")?;

    helper.assert_status("");

    Ok(())
}

#[test]
fn list_untracked_directories_that_indirectly_contain_files() -> Result<()> {
    let mut helper = CommandHelper::new();
    helper.init();

    helper.write_file("outer/inner/file.txt", "")?;

    helper.assert_status("?? outer/\n");

    Ok(())
}

fn setup_index_workspace_changes(helper: &mut CommandHelper) -> Result<()> {
    helper.write_file("1.txt", "one")?;
    helper.write_file("a/2.txt", "two")?;
    helper.write_file("a/b/3.txt", "three")?;
    helper.jit_cmd(&["add", "."]);
    helper.commit("commit message");

    Ok(())
}

#[test]
fn print_nothing_when_no_files_are_changed() -> Result<()> {
    let mut helper = CommandHelper::new();
    helper.init();
    setup_index_workspace_changes(&mut helper)?;

    helper.assert_status("");

    Ok(())
}

#[test]
fn report_files_with_modified_contents() -> Result<()> {
    let mut helper = CommandHelper::new();
    helper.init();
    setup_index_workspace_changes(&mut helper)?;

    helper.write_file("1.txt", "changed")?;
    helper.write_file("a/2.txt", "modified")?;

    helper.assert_status(
        " M 1.txt
 M a/2.txt
",
    );

    Ok(())
}
