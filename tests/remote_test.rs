mod common;

use assert_cmd::prelude::OutputAssertExt;
pub use common::CommandHelper;
use rstest::{fixture, rstest};

mod adding_a_remote {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.jit_cmd(&["remote", "add", "origin", "ssh://example.com/repo"]);

        helper
    }

    #[rstest]
    fn fail_to_add_an_existing_remote(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["remote", "add", "origin", "url"])
            .assert()
            .code(128)
            .stderr("fatal: remote origin already exists.\n");
    }

    #[rstest]
    fn list_the_remote(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["remote"])
            .assert()
            .code(0)
            .stdout("origin\n");
    }

    #[rstest]
    fn list_the_remote_with_its_urls(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["remote", "--verbose"])
            .assert()
            .code(0)
            .stdout(
                "\
origin\tssh://example.com/repo (fetch)
origin\tssh://example.com/repo (push)
",
            );
    }

    #[rstest]
    fn set_a_catch_all_fetch_refspec(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["config", "--local", "--get-all", "remote.origin.fetch"])
            .assert()
            .code(0)
            .stdout("+refs/heads/*:refs/remotes/origin/*\n");
    }
}

mod adding_a_remote_with_tracking_branches {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.jit_cmd(&[
            "remote",
            "add",
            "origin",
            "ssh://example.com/repo",
            "-t",
            "master",
            "-t",
            "topic",
        ]);

        helper
    }

    #[rstest]
    fn set_a_fetch_refspec_for_each_branch(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["config", "--local", "--get-all", "remote.origin.fetch"])
            .assert()
            .code(0)
            .stdout(
                "\
+refs/heads/master:refs/remotes/origin/master
+refs/heads/topic:refs/remotes/origin/topic
",
            );
    }
}

mod removing_a_remote {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.jit_cmd(&["remote", "add", "origin", "ssh://example.com/repo"]);

        helper
    }

    #[rstest]
    fn remove_the_remote(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["remote", "remove", "origin"])
            .assert()
            .code(0);

        helper.jit_cmd(&["remote"]).assert().code(0).stdout("");
    }

    #[rstest]
    fn fail_to_remove_a_missing_remote(mut helper: CommandHelper) {
        helper
            .jit_cmd(&["remote", "remove", "no-such"])
            .assert()
            .code(128)
            .stderr("fatal: No such remote: no-such\n");
    }
}
