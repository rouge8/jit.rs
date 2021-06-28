mod common;

use assert_cmd::prelude::OutputAssertExt;
pub use common::CommandHelper;
use jit::database::commit::Commit;
use jit::database::object::Object;
use jit::database::Database;
use jit::errors::Result;
use rstest::{fixture, rstest};

fn commit_file(helper: &mut CommandHelper, message: &str) -> Result<()> {
    helper.write_file("file.txt", message)?;
    helper.jit_cmd(&["add", "."]);
    helper.commit(message);

    Ok(())
}

///   o---o---o
///   A   B   C
mod with_a_chain_of_commits {
    use super::*;

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

    fn commits(helper: &CommandHelper) -> Vec<Commit> {
        ["@", "@^", "@^^"]
            .iter()
            .map(|rev| helper.load_commit(&rev).unwrap())
            .collect()
    }

    #[rstest]
    fn print_a_log_in_medium_format(mut helper: CommandHelper) {
        let commits = commits(&helper);

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
    fn print_a_log_in_medium_format_with_abbreviated_commit_ids(mut helper: CommandHelper) {
        let commits = commits(&helper);

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
    fn print_a_log_in_oneline_format(mut helper: CommandHelper) {
        let commits = commits(&helper);

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
    ) {
        let commits = commits(&helper);

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
    fn print_a_log_starting_from_a_specified_commit(mut helper: CommandHelper) {
        let commits = commits(&helper);

        helper
            .jit_cmd(&["log", "--pretty=oneline", "@^"])
            .assert()
            .code(0)
            .stdout(format!(
                "\
{} B
{} A\n",
                &commits[1].oid(),
                &commits[2].oid(),
            ));
    }

    #[rstest]
    fn print_a_log_with_short_decorations(mut helper: CommandHelper) {
        let commits = commits(&helper);

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
    fn print_a_log_with_detached_head(mut helper: CommandHelper) {
        let commits = commits(&helper);

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
    fn print_a_log_with_full_decorations(mut helper: CommandHelper) {
        let commits = commits(&helper);

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
    fn print_a_log_with_patches(mut helper: CommandHelper) {
        let commits = commits(&helper);

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

// m1  m2  m3
//  o---o---o [main]
//       \
//        o---o---o---o [topic]
//       t1  t2  t3  t4
mod with_a_tree_of_commits {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        // All commits to `main` will have the same timestamp
        helper
            .env
            .insert("GIT_AUTHOR_DATE", "Mon, 28 Jun 2021 18:04:07 +0000");

        for n in 1..=3 {
            commit_file(&mut helper, &format!("main-{}", n)).unwrap();
        }

        helper
            .jit_cmd(&["branch", "topic", "main^"])
            .assert()
            .code(0);
        helper.jit_cmd(&["checkout", "topic"]).assert().code(0);

        // Commits to `topic` will be one second later than those to main
        helper
            .env
            .insert("GIT_AUTHOR_DATE", "Mon, 28 Jun 2021 18:04:08 +0000");

        for n in 1..=4 {
            commit_file(&mut helper, &format!("topic-{}", n)).unwrap();
        }

        helper
    }

    fn main_commits(helper: &CommandHelper) -> Vec<String> {
        (0..=2)
            .map(|n| helper.resolve_revision(&format!("main~{}", n)).unwrap())
            .collect()
    }

    fn topic_commits(helper: &CommandHelper) -> Vec<String> {
        (0..=3)
            .map(|n| helper.resolve_revision(&format!("topic~{}", n)).unwrap())
            .collect()
    }

    #[rstest]
    fn log_the_combined_history_of_multiple_branches(mut helper: CommandHelper) {
        let main = main_commits(&helper);
        let topic = topic_commits(&helper);

        helper
            .jit_cmd(&[
                "log",
                "--pretty=oneline",
                "--decorate=short",
                "main",
                "topic",
            ])
            .assert()
            .code(0)
            .stdout(format!(
                "\
{} (HEAD -> topic) topic-4
{} topic-3
{} topic-2
{} topic-1
{} (main) main-3
{} main-2
{} main-1\n",
                topic[0], topic[1], topic[2], topic[3], main[0], main[1], main[2],
            ));
    }

    #[rstest]
    fn log_the_difference_from_one_branch_to_another(mut helper: CommandHelper) {
        let main = main_commits(&helper);
        let topic = topic_commits(&helper);

        helper
            .jit_cmd(&["log", "--pretty=oneline", "main..topic"])
            .assert()
            .code(0)
            .stdout(format!(
                "\
{} topic-4
{} topic-3
{} topic-2
{} topic-1\n",
                topic[0], topic[1], topic[2], topic[3],
            ));

        helper
            .jit_cmd(&["log", "--pretty=oneline", "main", "^topic"])
            .assert()
            .code(0)
            .stdout(format!("{} main-3\n", main[0]));
    }

    #[rstest]
    fn exclude_a_long_branch_when_commit_times_are_equal(mut helper: CommandHelper) -> Result<()> {
        let topic = topic_commits(&helper);

        helper.jit_cmd(&["branch", "side", "topic^^"]);
        helper.jit_cmd(&["checkout", "side"]);

        for n in 1..=10 {
            commit_file(&mut helper, &format!("side-{}", n))?;
        }

        helper
            .jit_cmd(&["log", "--pretty=oneline", "side..topic", "^main"])
            .assert()
            .code(0)
            .stdout(format!(
                "\
{} topic-4
{} topic-3\n",
                topic[0], topic[1],
            ));

        Ok(())
    }

    #[rstest]
    fn log_the_last_few_comnmits_on_a_branch(mut helper: CommandHelper) {
        let topic = topic_commits(&helper);

        helper
            .jit_cmd(&["log", "--pretty=oneline", "@~3.."])
            .assert()
            .code(0)
            .stdout(format!(
                "\
{} topic-4
{} topic-3
{} topic-2\n",
                topic[0], topic[1], topic[2],
            ));
    }
}
