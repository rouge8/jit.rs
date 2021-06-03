mod common;
use common::CommandHelper;
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