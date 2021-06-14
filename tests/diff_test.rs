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
",
        );

        Ok(())
    }
}
