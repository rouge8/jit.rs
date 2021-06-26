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
}
