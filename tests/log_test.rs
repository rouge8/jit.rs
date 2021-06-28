fn commit_file(helper: &mut CommandHelper, message: &str) -> Result<()> {
    helper.write_file("file.txt", message)?;
    helper.jit_cmd(&["add", "."]);
    helper.commit(message);

    Ok(())
}

    fn commits(helper: &CommandHelper) -> Vec<Commit> {
    fn print_a_log_in_medium_format(mut helper: CommandHelper) {
        let commits = commits(&helper);

    fn print_a_log_in_medium_format_with_abbreviated_commit_ids(mut helper: CommandHelper) {
        let commits = commits(&helper);

    fn print_a_log_in_oneline_format(mut helper: CommandHelper) {
        let commits = commits(&helper);

        let commits = commits(&helper);

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

    fn print_a_log_with_detached_head(mut helper: CommandHelper) {
        let commits = commits(&helper);

    fn print_a_log_with_full_decorations(mut helper: CommandHelper) {
        let commits = commits(&helper);


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