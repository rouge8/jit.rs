mod common;

use assert_cmd::prelude::OutputAssertExt;
pub use common::{helper, CommandHelper};
use jit::errors::Result;
use rstest::rstest;

fn assert_index(helper: &mut CommandHelper, expected: Vec<(u32, &str)>) -> Result<()> {
    helper.repo.index.load()?;

    let actual: Vec<(u32, &str)> = helper
        .repo
        .index
        .entries
        .values()
        .map(|entry| (entry.mode, entry.path.as_str()))
        .collect();

    assert_eq!(actual, expected);

    Ok(())
}

#[rstest]
fn add_a_regular_file_to_the_index(mut helper: CommandHelper) -> Result<()> {
    helper.write_file("hello.txt", "hello")?;

    helper.jit_cmd(&["add", "hello.txt"]);

    assert_index(&mut helper, vec![(0o100644, "hello.txt")]).unwrap();

    Ok(())
}

#[rstest]
fn add_an_executable_file_to_the_index(mut helper: CommandHelper) -> Result<()> {
    helper.write_file("hello.txt", "hello")?;
    helper.make_executable("hello.txt")?;

    helper.jit_cmd(&["add", "hello.txt"]);

    assert_index(&mut helper, vec![(0o100755, "hello.txt")]).unwrap();

    Ok(())
}

#[rstest]
fn add_multiple_files_to_the_index(mut helper: CommandHelper) -> Result<()> {
    helper.write_file("hello.txt", "hello")?;
    helper.write_file("world.txt", "world")?;

    helper.jit_cmd(&["add", "hello.txt", "world.txt"]);

    assert_index(
        &mut helper,
        vec![(0o100644, "hello.txt"), (0o100644, "world.txt")],
    )
    .unwrap();

    Ok(())
}

#[rstest]
fn incrementally_add_files_to_the_index(mut helper: CommandHelper) -> Result<()> {
    helper.write_file("hello.txt", "hello")?;
    helper.write_file("world.txt", "world")?;

    helper.jit_cmd(&["add", "hello.txt"]);

    assert_index(&mut helper, vec![(0o100644, "hello.txt")]).unwrap();

    helper.jit_cmd(&["add", "world.txt"]);

    assert_index(
        &mut helper,
        vec![(0o100644, "hello.txt"), (0o100644, "world.txt")],
    )
    .unwrap();

    Ok(())
}

#[rstest]
fn add_a_directory_to_the_index(mut helper: CommandHelper) -> Result<()> {
    helper.write_file("a-dir/nested.txt", "content")?;

    helper.jit_cmd(&["add", "a-dir"]);

    assert_index(&mut helper, vec![(0o100644, "a-dir/nested.txt")]).unwrap();

    Ok(())
}

#[rstest]
fn add_the_repository_root_to_the_index(mut helper: CommandHelper) -> Result<()> {
    helper.write_file("a/b/c/file.txt", "content")?;

    helper.jit_cmd(&["add", "."]);

    assert_index(&mut helper, vec![(0o100644, "a/b/c/file.txt")]).unwrap();

    Ok(())
}

#[rstest]
fn silent_on_success(mut helper: CommandHelper) -> Result<()> {
    helper.write_file("hello.txt", "hello")?;

    helper
        .jit_cmd(&["add", "hello.txt"])
        .assert()
        .code(0)
        .stdout("")
        .stderr("");

    Ok(())
}

#[rstest]
fn fail_for_non_existent_files(mut helper: CommandHelper) -> Result<()> {
    helper
        .jit_cmd(&["add", "no-such-file"])
        .assert()
        .code(128)
        .stdout("")
        .stderr("fatal: pathspec 'no-such-file' did not match any files\n");
    assert_index(&mut helper, vec![]).unwrap();

    Ok(())
}

#[rstest]
fn fail_for_unreadable_files(mut helper: CommandHelper) -> Result<()> {
    helper.write_file("secret.txt", "")?;
    helper.make_unreadable("secret.txt")?;

    helper
        .jit_cmd(&["add", "secret.txt"])
        .assert()
        .code(128)
        .stdout("")
        .stderr("error: open('secret.txt'): Permission denied\nfatal: adding files failed\n");
    assert_index(&mut helper, vec![]).unwrap();

    Ok(())
}

#[rstest]
fn fail_if_the_index_is_locked(mut helper: CommandHelper) -> Result<()> {
    helper.write_file("file.txt", "")?;
    helper.write_file(".git/index.lock", "")?;

    helper
        .jit_cmd(&["add", "file.txt"])
        .assert()
        .code(128)
        .stdout("");
    assert_index(&mut helper, vec![]).unwrap();

    Ok(())
}
