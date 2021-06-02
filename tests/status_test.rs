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
        "?? another.txt
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
