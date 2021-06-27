mod common;

use assert_cmd::prelude::OutputAssertExt;
pub use common::CommandHelper;
use jit::database::commit::Commit;
use jit::database::object::Object;
use jit::database::Database;
use jit::errors::Result;
use rstest::{fixture, rstest};

///   o---o---o
///   A   B   C
mod with_a_chain_of_commits {

    use super::*;

    fn commit_file(helper: &mut CommandHelper, message: &'static str) -> Result<()> {
        helper.write_file("file.txt", message)?;
        helper.jit_cmd(&["add", "."]);
        helper.commit(message);

        Ok(())
    }

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        for message in ["A", "B", "C"] {
            commit_file(&mut helper, message).unwrap();
        }

        helper.jit_cmd(&["branch", "topic", "@^^"]);

        helper
    }

    #[fixture]
    fn commits(helper: CommandHelper) -> Vec<Commit> {
        ["@", "@^", "@^^"]
            .iter()
            .map(|rev| helper.load_commit(&rev).unwrap())
            .collect()
    }

    #[rstest]
    fn print_a_log_in_medium_format(mut helper: CommandHelper, commits: Vec<Commit>) {
        helper.jit_cmd(&["log"]).assert().code(0).stdout(format!(
            "\
commit {}
Author: A. U. Thor <author@example.com>
Date:   {}

    C

commit {}
Author: A. U. Thor <author@example.com>
Date:   {}

    B

commit {}
Author: A. U. Thor <author@example.com>
Date:   {}

    A\n",
            commits[0].oid(),
            commits[0].author.readable_time(),
            commits[1].oid(),
            commits[1].author.readable_time(),
            commits[2].oid(),
            commits[2].author.readable_time(),
        ));
    }

    #[rstest]
    fn print_a_log_in_medium_format_with_abbreviated_commit_ids(
        mut helper: CommandHelper,
        commits: Vec<Commit>,
    ) {
        helper
            .jit_cmd(&["log", "--abbrev-commit"])
            .assert()
            .code(0)
            .stdout(format!(
                "\
commit {}
Author: A. U. Thor <author@example.com>
Date:   {}

    C

commit {}
Author: A. U. Thor <author@example.com>
Date:   {}

    B

commit {}
Author: A. U. Thor <author@example.com>
Date:   {}

    A\n",
                Database::short_oid(&commits[0].oid()),
                commits[0].author.readable_time(),
                Database::short_oid(&commits[1].oid()),
                commits[1].author.readable_time(),
                Database::short_oid(&commits[2].oid()),
                commits[2].author.readable_time(),
            ));
    }

    #[rstest]
    fn print_a_log_in_oneline_format(mut helper: CommandHelper, commits: Vec<Commit>) {
        helper
            .jit_cmd(&["log", "--oneline"])
            .assert()
            .code(0)
            .stdout(format!(
                "\
{} C
{} B
{} A\n",
                Database::short_oid(&commits[0].oid()),
                Database::short_oid(&commits[1].oid()),
                Database::short_oid(&commits[2].oid()),
            ));
    }

    #[rstest]
    #[case(vec!["log", "--pretty=oneline"])]
    #[case(vec!["log", "--oneline", "--no-abbrev-commit"])]
    fn print_a_log_in_oneline_format_without_abbreviated_commit_ids(
        #[case] cmd: Vec<&str>,
        mut helper: CommandHelper,
        commits: Vec<Commit>,
    ) {
        helper.jit_cmd(&cmd).assert().code(0).stdout(format!(
            "\
{} C
{} B
{} A\n",
            &commits[0].oid(),
            &commits[1].oid(),
            &commits[2].oid(),
        ));
    }

    #[rstest]
    fn print_a_log_with_short_decorations(mut helper: CommandHelper, commits: Vec<Commit>) {
        helper
            .jit_cmd(&["log", "--pretty=oneline", "--decorate=short"])
            .assert()
            .code(0)
            .stdout(format!(
                "\
{} (HEAD -> main) C
{} B
{} (topic) A\n",
                &commits[0].oid(),
                &commits[1].oid(),
                &commits[2].oid(),
            ));
    }

    #[rstest]
    fn print_a_log_with_detached_head(mut helper: CommandHelper, commits: Vec<Commit>) {
        helper.jit_cmd(&["checkout", "@"]);
        helper
            .jit_cmd(&["log", "--pretty=oneline", "--decorate=short"])
            .assert()
            .code(0)
            .stdout(format!(
                "\
{} (HEAD, main) C
{} B
{} (topic) A\n",
                &commits[0].oid(),
                &commits[1].oid(),
                &commits[2].oid(),
            ));
    }

    #[rstest]
    fn print_a_log_with_full_decorations(mut helper: CommandHelper, commits: Vec<Commit>) {
        helper
            .jit_cmd(&["log", "--pretty=oneline", "--decorate=full"])
            .assert()
            .code(0)
            .stdout(format!(
                "\
{} (HEAD -> refs/heads/main) C
{} B
{} (refs/heads/topic) A\n",
                &commits[0].oid(),
                &commits[1].oid(),
                &commits[2].oid(),
            ));
    }

    #[rstest]
    fn print_a_log_with_patches(mut helper: CommandHelper, commits: Vec<Commit>) {
        helper
            .jit_cmd(&["log", "--pretty=oneline", "--patch"])
            .assert()
            .code(0)
            .stdout(format!(
                "\
{} C
diff --git a/file.txt b/file.txt
index 7371f47..96d80cd 100644
--- a/file.txt
+++ b/file.txt
@@ -1,1 +1,1 @@
-B
+C
{} B
diff --git a/file.txt b/file.txt
index 8c7e5a6..7371f47 100644
--- a/file.txt
+++ b/file.txt
@@ -1,1 +1,1 @@
-A
+B
{} A
diff --git a/file.txt b/file.txt
new file mode 100644
index 0000000..8c7e5a6
--- /dev/null
+++ b/file.txt
@@ -0,0 +1,1 @@
+A\n",
                &commits[0].oid(),
                &commits[1].oid(),
                &commits[2].oid(),
            ));
    }
}
