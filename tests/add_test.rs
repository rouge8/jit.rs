mod common;

use assert_cmd::prelude::OutputAssertExt;
pub use common::CommandHelper;
use jit::errors::Result;

#[test]
fn add_a_regular_file_to_the_index() -> Result<()> {
    let mut helper = CommandHelper::new();
    helper.init();

    helper.write_file("hello.txt", "hello")?;

    helper.jit_cmd(&["add", "hello.txt"]);

    helper.assert_index(vec![(0o100644, "hello.txt")]).unwrap();

    Ok(())
}

#[test]
fn add_an_executable_file_to_the_index() -> Result<()> {
    let mut helper = CommandHelper::new();
    helper.init();

    helper.write_file("hello.txt", "hello")?;
    helper.make_executable("hello.txt")?;

    helper.jit_cmd(&["add", "hello.txt"]);

    helper.assert_index(vec![(0o100755, "hello.txt")]).unwrap();

    Ok(())
}

#[test]
fn add_multiple_files_to_the_index() -> Result<()> {
    let mut helper = CommandHelper::new();
    helper.init();

    helper.write_file("hello.txt", "hello")?;
    helper.write_file("world.txt", "world")?;

    helper.jit_cmd(&["add", "hello.txt", "world.txt"]);

    helper
        .assert_index(vec![(0o100644, "hello.txt"), (0o100644, "world.txt")])
        .unwrap();

    Ok(())
}

#[test]
fn incrementally_add_files_to_the_index() -> Result<()> {
    let mut helper = CommandHelper::new();
    helper.init();

    helper.write_file("hello.txt", "hello")?;
    helper.write_file("world.txt", "world")?;

    helper.jit_cmd(&["add", "hello.txt"]);

    helper.assert_index(vec![(0o100644, "hello.txt")]).unwrap();

    helper.jit_cmd(&["add", "world.txt"]);

    helper
        .assert_index(vec![(0o100644, "hello.txt"), (0o100644, "world.txt")])
        .unwrap();

    Ok(())
}

#[test]
fn add_a_directory_to_the_index() -> Result<()> {
    let mut helper = CommandHelper::new();
    helper.init();

    helper.write_file("a-dir/nested.txt", "content")?;

    helper.jit_cmd(&["add", "a-dir"]);

    helper
        .assert_index(vec![(0o100644, "a-dir/nested.txt")])
        .unwrap();

    Ok(())
}

#[test]
fn add_the_repository_root_to_the_index() -> Result<()> {
    let mut helper = CommandHelper::new();
    helper.init();

    helper.write_file("a/b/c/file.txt", "content")?;

    helper.jit_cmd(&["add", "."]);

    helper
        .assert_index(vec![(0o100644, "a/b/c/file.txt")])
        .unwrap();

    Ok(())
}

#[test]
fn silent_on_success() -> Result<()> {
    let mut helper = CommandHelper::new();
    helper.init();

    helper.write_file("hello.txt", "hello")?;

    helper
        .jit_cmd(&["add", "hello.txt"])
        .assert()
        .code(0)
        .stdout("")
        .stderr("");

    Ok(())
}

#[test]
fn fail_for_non_existent_files() -> Result<()> {
    let mut helper = CommandHelper::new();
    helper.init();

    helper
        .jit_cmd(&["add", "no-such-file"])
        .assert()
        .code(128)
        .stdout("")
        .stderr("fatal: pathspec 'no-such-file' did not match any files\n");
    helper.assert_index(vec![]).unwrap();

    Ok(())
}

#[test]
fn fail_for_unreadable_files() -> Result<()> {
    let mut helper = CommandHelper::new();
    helper.init();

    helper.write_file("secret.txt", "")?;
    helper.make_unreadable("secret.txt")?;

    helper
        .jit_cmd(&["add", "secret.txt"])
        .assert()
        .code(128)
        .stdout("")
        .stderr("error: open('secret.txt'): Permission denied\nfatal: adding files failed\n");
    helper.assert_index(vec![]).unwrap();

    Ok(())
}

#[test]
fn fail_if_the_index_is_locked() -> Result<()> {
    let mut helper = CommandHelper::new();
    helper.init();

    helper.write_file("file.txt", "")?;
    helper.write_file(".git/index.lock", "")?;

    helper
        .jit_cmd(&["add", "file.txt"])
        .assert()
        .code(128)
        .stdout("");
    helper.assert_index(vec![]).unwrap();

    Ok(())
}
