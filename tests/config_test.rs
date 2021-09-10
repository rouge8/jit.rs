mod common;

use assert_cmd::prelude::OutputAssertExt;
pub use common::CommandHelper;
use rstest::{fixture, rstest};

#[fixture]
fn helper() -> CommandHelper {
    let mut helper = CommandHelper::new();
    helper.init();

    helper
}

#[rstest]
fn return_1_for_unknown_variables(mut helper: CommandHelper) {
    helper
        .jit_cmd(&["config", "--local", "no.such"])
        .assert()
        .code(1);
}

#[rstest]
fn return_1_when_the_key_is_invalid(mut helper: CommandHelper) {
    helper
        .jit_cmd(&["config", "--local", "0.0"])
        .assert()
        .code(1)
        .stderr("error: invalid key: 0.0\n");
}

#[rstest]
fn return_2_when_no_section_is_given(mut helper: CommandHelper) {
    helper
        .jit_cmd(&["config", "--local", "no"])
        .assert()
        .code(2)
        .stderr("error: key does not contain a section: no\n");
}

#[rstest]
fn return_the_value_of_a_set_variable(mut helper: CommandHelper) {
    helper.jit_cmd(&["config", "core.editor", "ed"]);

    helper
        .jit_cmd(&["config", "--local", "Core.Editor"])
        .assert()
        .code(0)
        .stdout("ed\n");
}

#[rstest]
fn return_the_value_of_a_set_variable_in_a_subsection(mut helper: CommandHelper) {
    helper.jit_cmd(&["config", "remote.origin.url", "git@github.com:jcoglan.jit"]);

    helper
        .jit_cmd(&["config", "--local", "Remote.origin.URL"])
        .assert()
        .code(0)
        .stdout("git@github.com:jcoglan.jit\n");
}

#[rstest]
fn unset_a_variable(mut helper: CommandHelper) {
    helper.jit_cmd(&["config", "core.editor", "ed"]);
    helper.jit_cmd(&["config", "--unset", "core.editor"]);

    helper
        .jit_cmd(&["config", "--local", "Core.Editor"])
        .assert()
        .code(1);
}

mod with_multi_valued_variables {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.jit_cmd(&["config", "--add", "remote.origin.fetch", "master"]);
        helper.jit_cmd(&["config", "--add", "remote.origin.fetch", "topic"]);

        helper
    }

    #[rstest]
    fn return_the_last_value(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["config", "remote.origin.fetch"])
            .assert()
            .code(0)
            .stdout("topic\n");
    }

    #[rstest]
    fn return_all_the_values(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["config", "--get-all", "remote.origin.fetch"])
            .assert()
            .code(0)
            .stdout(
                "\
master
topic
",
            );
    }

    #[rstest]
    fn return_5_on_trying_to_set_a_variable(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["config", "remote.origin.fetch", "new-value"])
            .assert()
            .code(5);

        helper
            .jit_cmd(&["config", "--get-all", "remote.origin.fetch"])
            .assert()
            .code(0)
            .stdout(
                "\
master
topic
",
            );
    }

    #[rstest]
    fn replace_a_variable(mut helper: CommandHelper) {
        helper.jit_cmd(&[
            "config",
            "--replace-all",
            "remote.origin.fetch",
            "new-value",
        ]);

        helper
            .jit_cmd(&["config", "--get-all", "remote.origin.fetch"])
            .assert()
            .code(0)
            .stdout("new-value\n");
    }

    #[rstest]
    fn return_5_on_trying_to_unset_a_variable(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["config", "--unset", "remote.origin.fetch"])
            .assert()
            .code(5);

        helper
            .jit_cmd(&["config", "--get-all", "remote.origin.fetch"])
            .assert()
            .code(0)
            .stdout(
                "\
master
topic
",
            );
    }

    #[rstest]
    fn unset_a_variable(mut helper: CommandHelper) {
        helper.jit_cmd(&["config", "--unset-all", "remote.origin.fetch"]);

        helper
            .jit_cmd(&["config", "--get-all", "remote.origin.fetch"])
            .assert()
            .code(1);
    }
}

#[rstest]
fn remove_a_section(mut helper: CommandHelper) {
    helper.jit_cmd(&["config", "core.editor", "ed"]);
    helper.jit_cmd(&["config", "remote.origin.url", "ssh://example.com/repo"]);
    helper.jit_cmd(&["config", "--remove-section", "core"]);

    helper
        .jit_cmd(&["config", "--local", "remote.origin.url"])
        .assert()
        .code(0)
        .stdout("ssh://example.com/repo\n");

    helper
        .jit_cmd(&["config", "--local", "core.editor"])
        .assert()
        .code(1);
}

#[rstest]
fn remove_a_subsection(mut helper: CommandHelper) {
    helper.jit_cmd(&["config", "core.editor", "ed"]);
    helper.jit_cmd(&["config", "remote.origin.url", "ssh://example.com/repo"]);
    helper.jit_cmd(&["config", "--remove-section", "remote.origin"]);

    helper
        .jit_cmd(&["config", "--local", "core.editor"])
        .assert()
        .code(0)
        .stdout("ed\n");

    helper
        .jit_cmd(&["config", "--local", "remote.origin.url"])
        .assert()
        .code(1);
}
