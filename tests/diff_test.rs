mod common;

pub use common::{helper, CommandHelper};
use jit::errors::Result;
use rstest::{fixture, rstest};

mod with_a_file_in_the_index {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.write_file("file.txt", "contents").unwrap();
        helper.jit_cmd(&["add", "."]);

        helper
    }

    #[rstest]
    fn diff_a_file_with_modified_contents(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("file.txt", "changed")?;

        helper.assert_diff(
            "\
diff --git a/file.txt b/file.txt
index 0839b2e..21fb1ec 100644
--- a/file.txt
+++ b/file.txt
@@ -1,1 +1,1 @@
-contents
+changed
",
        );

        Ok(())
    }

    #[rstest]
    fn diff_a_file_with_changed_mode(mut helper: CommandHelper) -> Result<()> {
        helper.make_executable("file.txt")?;

        helper.assert_diff(
            "\
diff --git a/file.txt b/file.txt
old mode 100644
new mode 100755
",
        );

        Ok(())
    }

    #[rstest]
    fn diff_a_file_with_changed_mode_and_contents(mut helper: CommandHelper) -> Result<()> {
        helper.make_executable("file.txt")?;

        helper.write_file("file.txt", "changed")?;

        helper.assert_diff(
            "\
diff --git a/file.txt b/file.txt
old mode 100644
new mode 100755
index 0839b2e..21fb1ec
--- a/file.txt
+++ b/file.txt
@@ -1,1 +1,1 @@
-contents
+changed
",
        );

        Ok(())
    }

    #[rstest]
    fn diff_a_deleted_file(mut helper: CommandHelper) -> Result<()> {
        helper.delete("file.txt")?;

        helper.assert_diff(
            "\
diff --git a/file.txt b/file.txt
deleted file mode 100644
index 0839b2e..0000000
--- a/file.txt
+++ /dev/null
@@ -1,1 +0,0 @@
-contents
",
        );

        Ok(())
    }
}

mod with_a_head_commit {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.write_file("file.txt", "contents").unwrap();
        helper.jit_cmd(&["add", "."]);
        helper.commit("first commit");

        helper
    }

    #[rstest]
    fn diff_a_file_with_modified_contents(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("file.txt", "changed")?;
        helper.jit_cmd(&["add", "."]);

        // This write will not be present in the diff
        helper.write_file("file.txt", "changed again")?;

        helper.assert_diff_cached(
            "\
diff --git a/file.txt b/file.txt
index 0839b2e..21fb1ec 100644
--- a/file.txt
+++ b/file.txt
@@ -1,1 +1,1 @@
-contents
+changed
",
        );

        Ok(())
    }

    #[rstest]
    fn diff_a_file_with_changed_mode(mut helper: CommandHelper) -> Result<()> {
        helper.make_executable("file.txt")?;
        helper.jit_cmd(&["add", "."]);

        helper.assert_diff_cached(
            "\
diff --git a/file.txt b/file.txt
old mode 100644
new mode 100755
",
        );

        Ok(())
    }

    #[rstest]
    fn diff_a_file_with_changed_mode_and_contents(mut helper: CommandHelper) -> Result<()> {
        helper.make_executable("file.txt")?;

        helper.write_file("file.txt", "changed")?;
        helper.jit_cmd(&["add", "."]);

        // This write will not be present in the diff
        helper.write_file("file.txt", "changed again")?;

        helper.assert_diff_cached(
            "\
diff --git a/file.txt b/file.txt
old mode 100644
new mode 100755
index 0839b2e..21fb1ec
--- a/file.txt
+++ b/file.txt
@@ -1,1 +1,1 @@
-contents
+changed
",
        );

        Ok(())
    }

    #[rstest]
    fn diff_a_deleted_file(mut helper: CommandHelper) -> Result<()> {
        helper.delete("file.txt")?;
        helper.delete(".git/index")?;
        helper.jit_cmd(&["add", "."]);

        helper.assert_diff_cached(
            "\
diff --git a/file.txt b/file.txt
deleted file mode 100644
index 0839b2e..0000000
--- a/file.txt
+++ /dev/null
@@ -1,1 +0,0 @@
-contents
",
        );

        Ok(())
    }

    #[rstest]
    fn diff_an_added_file(mut helper: CommandHelper) -> Result<()> {
        helper.write_file("another.txt", "hello")?;
        helper.jit_cmd(&["add", "."]);

        helper.assert_diff_cached(
            "\
diff --git a/another.txt b/another.txt
new file mode 100644
index 0000000..b6fc4c6
--- /dev/null
+++ b/another.txt
@@ -0,0 +1,1 @@
+hello
",
        );

        Ok(())
    }
}
