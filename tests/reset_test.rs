mod common;

pub use common::CommandHelper;
use jit::errors::Result;
use rstest::{fixture, rstest};
use std::collections::HashMap;

fn assert_index(helper: &mut CommandHelper, contents: &HashMap<&str, &str>) -> Result<()> {
    let mut files = HashMap::new();

    let mut repo = helper.repo();
    repo.index.load()?;

    for entry in repo.index.entries.values() {
        let blob = repo.database.load_blob(&entry.oid)?;
        files.insert(
            entry.path.clone(),
            std::str::from_utf8(&blob.data)
                .expect("Invalid UTF-8")
                .to_string(),
        );
    }

    let contents: HashMap<_, _> = contents
        .iter()
        .map(|(key, val)| (key.to_string(), val.to_string()))
        .collect();
    assert_eq!(files, contents);

    Ok(())
}

mod with_no_head_commit {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.write_file("a.txt", "1").unwrap();
        helper.write_file("outer/b.txt", "2").unwrap();
        helper.write_file("outer/inner/c.txt", "3").unwrap();

        helper.jit_cmd(&["add", "."]);

        helper
    }

    fn assert_unchanged_workspace(helper: &CommandHelper) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("a.txt", "1");
        workspace.insert("outer/b.txt", "2");
        workspace.insert("outer/inner/c.txt", "3");
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn remove_a_single_file_from_the_index(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["reset", "a.txt"]);

        let mut index = HashMap::new();
        index.insert("outer/b.txt", "2");
        index.insert("outer/inner/c.txt", "3");
        assert_index(&mut helper, &index)?;

        assert_unchanged_workspace(&helper)?;

        Ok(())
    }

    #[rstest]
    fn remove_a_directory_from_the_index(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["reset", "outer"]);

        let mut index = HashMap::new();
        index.insert("a.txt", "1");
        assert_index(&mut helper, &index)?;

        assert_unchanged_workspace(&helper)?;

        Ok(())
    }
}

mod with_a_head_commit {
    use super::*;

    #[fixture]
    fn helper() -> CommandHelper {
        let mut helper = CommandHelper::new();
        helper.init();

        helper.write_file("a.txt", "1").unwrap();
        helper.write_file("outer/b.txt", "2").unwrap();
        helper.write_file("outer/inner/c.txt", "3").unwrap();

        helper.jit_cmd(&["add", "."]);
        helper.commit("first");

        helper.jit_cmd(&["rm", "a.txt"]);
        helper.write_file("outer/d.txt", "4").unwrap();
        helper.write_file("outer/inner/c.txt", "5").unwrap();
        helper.jit_cmd(&["add", "."]);
        helper.write_file("outer/e.txt", "6").unwrap();

        helper
    }

    fn assert_unchanged_workspace(helper: &CommandHelper) -> Result<()> {
        let mut workspace = HashMap::new();
        workspace.insert("outer/b.txt", "2");
        workspace.insert("outer/d.txt", "4");
        workspace.insert("outer/e.txt", "6");
        workspace.insert("outer/inner/c.txt", "5");
        helper.assert_workspace(&workspace)?;

        Ok(())
    }

    #[rstest]
    fn restore_a_file_removed_from_the_index(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["reset", "a.txt"]);

        let mut index = HashMap::new();
        index.insert("a.txt", "1");
        index.insert("outer/b.txt", "2");
        index.insert("outer/d.txt", "4");
        index.insert("outer/inner/c.txt", "5");
        assert_index(&mut helper, &index)?;

        assert_unchanged_workspace(&helper)?;

        Ok(())
    }

    #[rstest]
    fn reset_a_file_modified_in_the_index(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["reset", "outer/inner"]);

        let mut index = HashMap::new();
        index.insert("outer/b.txt", "2");
        index.insert("outer/d.txt", "4");
        index.insert("outer/inner/c.txt", "3");
        assert_index(&mut helper, &index)?;

        assert_unchanged_workspace(&helper)?;

        Ok(())
    }

    #[rstest]
    fn remove_a_file_added_to_the_index(mut helper: CommandHelper) -> Result<()> {
        helper.jit_cmd(&["reset", "outer/d.txt"]);

        let mut index = HashMap::new();
        index.insert("outer/b.txt", "2");
        index.insert("outer/inner/c.txt", "5");
        assert_index(&mut helper, &index)?;

        assert_unchanged_workspace(&helper)?;

        Ok(())
    }
}
