mod common;

use assert_cmd::prelude::OutputAssertExt;
use chrono::{Duration, Local};
pub use common::CommandHelper;
use jit::database::commit::Commit;
use jit::database::object::Object;
use jit::database::Database;
use jit::errors::Result;
use rstest::{fixture, rstest};
use std::collections::HashMap;

fn commit_file(helper: &mut CommandHelper, message: &str) -> Result<()> {
    helper.write_file("file.txt", message)?;
    helper.jit_cmd(&["add", "."]);
    helper.commit(message);

    Ok(())
}

fn commit_tree(
    helper: &mut CommandHelper,
    message: &str,
    files: HashMap<&'static str, &str>,
) -> Result<()> {
    for (path, contents) in files {
        helper.write_file(path, contents)?;
    }
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

mod with_commits_changing_different_files {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let mut tree_first = HashMap::new();
        tree_first.insert("a/1.txt", "1");
        tree_first.insert("b/c/2.txt", "2");
        commit_tree(&mut helper, "first", tree_first).unwrap();

        let mut tree_second = HashMap::new();
        tree_second.insert("a/1.txt", "10");
        tree_second.insert("b/3.txt", "3");
        commit_tree(&mut helper, "second", tree_second).unwrap();

        let mut tree_third = HashMap::new();
        tree_third.insert("b/c/2.txt", "4");
        commit_tree(&mut helper, "third", tree_third).unwrap();

        helper
    }

    fn commits(helper: &CommandHelper) -> Vec<Commit> {
        ["@^^", "@^", "@"]
            .iter()
            .map(|rev| helper.load_commit(&rev).unwrap())
            .collect()
    }

    #[rstest]
    fn log_commits_that_change_a_file(mut helper: CommandHelper) {
        let commits = commits(&helper);

        helper
            .jit_cmd(&["log", "--pretty=oneline", "a/1.txt"])
            .assert()
            .code(0)
            .stdout(format!(
                "\
{} second
{} first\n",
                commits[1].oid(),
                commits[0].oid(),
            ));
    }

    #[rstest]
    fn log_commits_that_change_a_directory(mut helper: CommandHelper) {
        let commits = commits(&helper);

        helper
            .jit_cmd(&["log", "--pretty=oneline", "b"])
            .assert()
            .code(0)
            .stdout(format!(
                "\
{} third
{} second
{} first\n",
                commits[2].oid(),
                commits[1].oid(),
                commits[0].oid(),
            ));
    }

    #[rstest]
    fn log_commits_that_change_a_directory_and_one_of_its_files(mut helper: CommandHelper) {
        let commits = commits(&helper);

        helper
            .jit_cmd(&["log", "--pretty=oneline", "b", "b/3.txt"])
            .assert()
            .code(0)
            .stdout(format!(
                "\
{} third
{} second
{} first\n",
                commits[2].oid(),
                commits[1].oid(),
                commits[0].oid(),
            ));
    }

    #[rstest]
    fn log_commits_that_change_a_nested_directory(mut helper: CommandHelper) {
        let commits = commits(&helper);

        helper
            .jit_cmd(&["log", "--pretty=oneline", "b/c"])
            .assert()
            .code(0)
            .stdout(format!(
                "\
{} third
{} first\n",
                commits[2].oid(),
                commits[0].oid(),
            ));
    }

    #[rstest]
    fn log_commits_with_patches_for_selected_files(mut helper: CommandHelper) {
        let commits = commits(&helper);

        helper
            .jit_cmd(&["log", "--pretty=oneline", "--patch", "a/1.txt"])
            .assert()
            .code(0)
            .stdout(format!(
                "\
{} second
diff --git a/a/1.txt b/a/1.txt
index 56a6051..9a03714 100644
--- a/a/1.txt
+++ b/a/1.txt
@@ -1,1 +1,1 @@
-1
+10
{} first
diff --git a/a/1.txt b/a/1.txt
new file mode 100644
index 0000000..56a6051
--- /dev/null
+++ b/a/1.txt
@@ -0,0 +1,1 @@
+1\n",
                commits[1].oid(),
                commits[0].oid(),
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
        helper.env.insert(
            String::from("GIT_AUTHOR_DATE"),
            String::from("Mon, 28 Jun 2021 18:04:07 +0000"),
        );

        for n in 1..=3 {
            commit_file(&mut helper, &format!("main-{}", n)).unwrap();
        }

        helper
            .jit_cmd(&["branch", "topic", "main^"])
            .assert()
            .code(0);
        helper.jit_cmd(&["checkout", "topic"]).assert().code(0);

        // Commits to `topic` will be one second later than those to main
        helper.env.insert(
            String::from("GIT_AUTHOR_DATE"),
            String::from("Mon, 28 Jun 2021 18:04:08 +0000"),
        );

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

///   A   B   C   D   J   K
///   o---o---o---o---o---o [main]
///        \         /
///         o---o---o---o [topic]
///         E   F   G   H
mod with_a_graph_of_commits {
    use super::*;

    #[fixture]
    fn base_helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        let time = Local::now();

        helper
            .env
            .insert(String::from("GIT_AUTHOR_DATE"), time.to_rfc2822());
        let mut tree = HashMap::new();
        tree.insert("f.txt", "0");
        tree.insert("g.txt", "0");
        commit_tree(&mut helper, "A", tree).unwrap();
        let mut tree = HashMap::new();
        tree.insert("f.txt", "B");
        tree.insert("h.txt", "one\ntwo\nthree\n");
        commit_tree(&mut helper, "B", tree).unwrap();

        helper.env.insert(
            String::from("GIT_AUTHOR_DATE"),
            (time + Duration::seconds(1)).to_rfc2822(),
        );
        for n in ["C", "D"] {
            let mut tree = HashMap::new();
            tree.insert("f.txt", n);
            let h = format!("{}\ntwo\nthree\n", n);
            tree.insert("h.txt", h.as_str());
            commit_tree(&mut helper, n, tree).unwrap();
        }

        helper.jit_cmd(&["branch", "topic", "main~2"]);
        helper.jit_cmd(&["checkout", "topic"]);

        helper.env.insert(
            String::from("GIT_AUTHOR_DATE"),
            (time + Duration::seconds(2)).to_rfc2822(),
        );
        for n in ["E", "F", "G", "H"] {
            let mut tree = HashMap::new();
            tree.insert("g.txt", n);
            let h = format!("one\ntwo\n{}\n", n);
            tree.insert("h.txt", h.as_str());
            commit_tree(&mut helper, n, tree).unwrap();
        }

        helper.jit_cmd(&["checkout", "main"]);
        helper.stdin = String::from("J");
        helper.jit_cmd(&["merge", "topic^"]).assert().code(0);

        helper.env.insert(
            String::from("GIT_AUTHOR_DATE"),
            (time + Duration::seconds(3)).to_rfc2822(),
        );
        let mut tree = HashMap::new();
        tree.insert("f.txt", "K");
        commit_tree(&mut helper, "K", tree).unwrap();

        helper
    }

    #[fixture]
    fn helper(base_helper: CommandHelper) -> CommandHelper {
        base_helper
    }

    fn main_commits(helper: &CommandHelper) -> Vec<String> {
        (0..=5)
            .map(|n| helper.resolve_revision(&format!("main~{}", n)).unwrap())
            .collect()
    }

    fn topic_commits(helper: &CommandHelper) -> Vec<String> {
        (0..=3)
            .map(|n| helper.resolve_revision(&format!("topic~{}", n)).unwrap())
            .collect()
    }

    #[rstest]
    fn log_concurrent_branches_leading_to_a_merge(mut helper: CommandHelper) {
        let main = main_commits(&helper);
        let topic = topic_commits(&helper);

        helper
            .jit_cmd(&["log", "--pretty=oneline"])
            .assert()
            .code(0)
            .stdout(format!(
                "\
{} K
{} J
{} G
{} F
{} E
{} D
{} C
{} B
{} A
",
                main[0], main[1], topic[1], topic[2], topic[3], main[2], main[3], main[4], main[5],
            ));
    }

    #[rstest]
    fn log_the_first_parent_of_a_merge(mut helper: CommandHelper) {
        let main = main_commits(&helper);

        helper
            .jit_cmd(&["log", "--pretty=oneline", "main^^"])
            .assert()
            .code(0)
            .stdout(format!(
                "\
{} D
{} C
{} B
{} A
",
                main[2], main[3], main[4], main[5],
            ));
    }

    #[rstest]
    fn log_the_second_parent_of_a_merge(mut helper: CommandHelper) {
        let main = main_commits(&helper);
        let topic = topic_commits(&helper);

        helper
            .jit_cmd(&["log", "--pretty=oneline", "main^^2"])
            .assert()
            .code(0)
            .stdout(format!(
                "\
{} G
{} F
{} E
{} B
{} A
",
                topic[1], topic[2], topic[3], main[4], main[5],
            ));
    }

    #[rstest]
    fn log_unmerged_commits_on_a_branch(mut helper: CommandHelper) {
        let topic = topic_commits(&helper);

        helper
            .jit_cmd(&["log", "--pretty=oneline", "main..topic"])
            .assert()
            .code(0)
            .stdout(format!("{} H\n", topic[0]));
    }

    #[rstest]
    fn do_not_show_patches_for_merge_commits(mut helper: CommandHelper) {
        let main = main_commits(&helper);

        helper
            .jit_cmd(&[
                "log",
                "--pretty=oneline",
                "--patch",
                "topic..main",
                "^main^^^",
            ])
            .assert()
            .code(0)
            .stdout(format!(
                "\
{} K
diff --git a/f.txt b/f.txt
index 02358d2..449e49e 100644
--- a/f.txt
+++ b/f.txt
@@ -1,1 +1,1 @@
-D
+K
{} J
{} D
diff --git a/f.txt b/f.txt
index 96d80cd..02358d2 100644
--- a/f.txt
+++ b/f.txt
@@ -1,1 +1,1 @@
-C
+D
diff --git a/h.txt b/h.txt
index 4e5ce14..4139691 100644
--- a/h.txt
+++ b/h.txt
@@ -1,3 +1,3 @@
-C
+D
 two
 three
",
                main[0], main[1], main[2]
            ));
    }

    #[rstest]
    fn show_combined_patches_for_merges(mut helper: CommandHelper) {
        let main = main_commits(&helper);

        helper
            .jit_cmd(&["log", "--pretty=oneline", "--cc", "topic..main", "^main^^^"])
            .assert()
            .code(0)
            .stdout(format!(
                "\
{} K
diff --git a/f.txt b/f.txt
index 02358d2..449e49e 100644
--- a/f.txt
+++ b/f.txt
@@ -1,1 +1,1 @@
-D
+K
{} J
diff --cc h.txt
index 4139691,f3e97ee..4e78f4f
--- a/h.txt
+++ b/h.txt
@@@ -1,3 -1,3 +1,3 @@@
 -one
 +D
  two
- three
+ G
{} D
diff --git a/f.txt b/f.txt
index 96d80cd..02358d2 100644
--- a/f.txt
+++ b/f.txt
@@ -1,1 +1,1 @@
-C
+D
diff --git a/h.txt b/h.txt
index 4e5ce14..4139691 100644
--- a/h.txt
+++ b/h.txt
@@ -1,3 +1,3 @@
-C
+D
 two
 three
",
                main[0], main[1], main[2]
            ));
    }

    #[rstest]
    fn do_not_list_merges_with_treesame_parents_for_prune_paths(mut helper: CommandHelper) {
        let main = main_commits(&helper);
        let topic = topic_commits(&helper);

        helper
            .jit_cmd(&["log", "--pretty=oneline", "g.txt"])
            .assert()
            .code(0)
            .stdout(format!(
                "\
{} G
{} F
{} E
{} A
",
                topic[1], topic[2], topic[3], main[5]
            ));
    }

    mod with_changes_that_are_undone_on_a_branch_leading_to_a_merge {
        use super::*;

        #[fixture]
        fn helper(mut base_helper: CommandHelper) -> CommandHelper {
            let time = Local::now();

            base_helper
                .env
                .insert(String::from("GIT_AUTHOR_DATE"), time.to_rfc2822());

            base_helper.jit_cmd(&["branch", "aba", "main~4"]);
            base_helper.jit_cmd(&["checkout", "aba"]);

            base_helper.env.insert(
                String::from("GIT_AUTHOR_DATE"),
                (time + Duration::seconds(1)).to_rfc2822(),
            );
            for n in ["C", "0"] {
                let mut tree = HashMap::new();
                tree.insert("g.txt", n);
                commit_tree(&mut base_helper, n, tree).unwrap();
            }

            base_helper.stdin = String::from("J");
            base_helper.jit_cmd(&["merge", "topic^"]).assert().code(0);

            base_helper.env.insert(
                String::from("GIT_AUTHOR_DATE"),
                (time + Duration::seconds(3)).to_rfc2822(),
            );
            let mut tree = HashMap::new();
            tree.insert("f.txt", "K");
            commit_tree(&mut base_helper, "K", tree).unwrap();

            base_helper
        }

        #[rstest]
        fn do_not_list_commits_on_the_filtered_branch(mut helper: CommandHelper) {
            let main = main_commits(&helper);
            let topic = topic_commits(&helper);

            helper
                .jit_cmd(&["log", "--pretty=oneline", "g.txt"])
                .assert()
                .code(0)
                .stdout(format!(
                    "\
{} G
{} F
{} E
{} A
",
                    topic[1], topic[2], topic[3], main[5],
                ));
        }
    }
}
