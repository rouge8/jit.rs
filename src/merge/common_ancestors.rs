use crate::database::commit::Commit;
use crate::database::object::Object;
use crate::database::Database;
use crate::errors::Result;
use lazy_static::lazy_static;
use std::collections::{HashMap, HashSet, VecDeque};

lazy_static! {
    static ref BOTH_PARENTS: HashSet<Flag> = {
        let mut v = HashSet::new();
        v.insert(Flag::Parent1);
        v.insert(Flag::Parent2);

        v
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Flag {
    Parent1,
    Parent2,
}

#[derive(Debug)]
pub struct CommonAncestors<'a> {
    database: &'a Database,
    flags: HashMap<String, HashSet<Flag>>,
    queue: VecDeque<Commit>,
}

impl<'a> CommonAncestors<'a> {
    pub fn new(database: &'a Database, one: String, two: String) -> Result<Self> {
        let mut queue = VecDeque::new();
        let mut flags = HashMap::new();

        Self::insert_by_date(&mut queue, database.load_commit(&one)?);
        let mut one_flags = HashSet::new();
        one_flags.insert(Flag::Parent1);
        flags.insert(one, one_flags);

        Self::insert_by_date(&mut queue, database.load_commit(&two)?);
        // Use `flags.entry(two)` to grab the existing set of flags if `one == two`.
        let two_flags = flags.entry(two).or_insert_with(HashSet::new);
        two_flags.insert(Flag::Parent2);

        Ok(Self {
            database,
            flags,
            queue,
        })
    }

    pub fn find(&mut self) -> Result<Option<String>> {
        while !self.queue.is_empty() {
            let commit = self.queue.pop_front().unwrap();
            let flags = self.flags[&commit.oid()].clone();

            if flags == *BOTH_PARENTS {
                return Ok(Some(commit.oid()));
            }

            self.add_parents(&commit, flags)?;
        }

        Ok(None)
    }

    fn add_parents(&mut self, commit: &Commit, flags: HashSet<Flag>) -> Result<()> {
        if commit.parent.is_none() {
            return Ok(());
        }

        let parent = self
            .database
            .load_commit(&commit.parent.as_ref().unwrap())?;

        let current_flags = self.flags.entry(parent.oid()).or_insert_with(HashSet::new);
        if current_flags.is_superset(&flags) {
            return Ok(());
        }

        for flag in flags {
            current_flags.insert(flag);
        }
        Self::insert_by_date(&mut self.queue, parent);

        Ok(())
    }

    fn insert_by_date(list: &mut VecDeque<Commit>, commit: Commit) {
        let index = list.iter().position(|c| c.date() < commit.date());
        if let Some(index) = index {
            list.insert(index, commit);
        } else {
            list.push_back(commit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::author::Author;
    use crate::database::commit::Commit;
    use crate::database::object::Object;
    use chrono::{DateTime, FixedOffset, Local};
    use rstest::{fixture, rstest};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    struct GraphHelper {
        db_path: PathBuf,
        database: Database,
        time: DateTime<FixedOffset>,
        commits: HashMap<String, String>,
    }

    impl GraphHelper {
        pub fn new() -> Self {
            let tmp_dir = TempDir::new().unwrap();
            let db_path = tmp_dir.into_path().canonicalize().unwrap();
            let now = Local::now();

            Self {
                db_path: db_path.clone(),
                database: Database::new(db_path),
                time: now.with_timezone(now.offset()),
                commits: HashMap::new(),
            }
        }

        pub fn commit(&mut self, parent: Option<&str>, message: &str) -> Result<()> {
            let author = Author::new(
                String::from("A. U. Thor"),
                String::from("author@example.com"),
                self.time,
            );
            let parent = if let Some(parent) = parent {
                self.commits.get(parent).map(|parent| parent.to_owned())
            } else {
                None
            };
            let commit = Commit::new(parent, "0".repeat(40), author, message.to_string());

            self.database.store(&commit)?;
            self.commits.insert(message.to_string(), commit.oid());

            Ok(())
        }

        pub fn chain(&mut self, names: &[Option<&str>]) -> Result<()> {
            for window in names.windows(2) {
                let parent = window[0];
                let message = window[1].unwrap();

                self.commit(parent, message)?;
            }

            Ok(())
        }

        pub fn ancestor(&self, left: &str, right: &str) -> Result<String> {
            let mut common = CommonAncestors::new(
                &self.database,
                self.commits[left].clone(),
                self.commits[right].clone(),
            )?;
            let message = self.database.load_commit(&common.find()?.unwrap())?.message;

            Ok(message)
        }
    }

    impl Drop for GraphHelper {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.db_path).unwrap();
        }
    }

    /// o---o---o---o
    /// A   B   C   D
    mod with_a_linear_history {
        use super::*;

        #[fixture]
        fn helper() -> GraphHelper {
            let mut helper = GraphHelper::new();

            helper
                .chain(&[None, Some("A"), Some("B"), Some("C"), Some("D")])
                .unwrap();

            helper
        }

        #[rstest]
        fn find_the_common_ancestor_of_a_commit_with_itself(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("D", "D")?, "D");

            Ok(())
        }

        #[rstest]
        fn find_the_commit_that_is_an_ancestor_of_the_other(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("B", "D")?, "B");

            Ok(())
        }

        #[rstest]
        fn find_the_same_commit_if_the_arguments_are_reversed(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("D", "B")?, "B");

            Ok(())
        }

        #[rstest]
        fn find_a_root_commit(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("A", "C")?, "A");

            Ok(())
        }

        #[rstest]
        fn find_the_intersection_of_a_root_commit_with_itself(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("A", "A")?, "A");

            Ok(())
        }
    }

    ///          E   F   G   H
    ///          o---o---o---o
    ///         /         \
    ///        /  C   D    \
    ///   o---o---o---o     o---o
    ///   A   B    \        J   K
    ///             \
    ///              o---o---o
    ///              L   M   N
    mod with_a_forking_history {
        use super::*;

        #[fixture]
        fn helper() -> GraphHelper {
            let mut helper = GraphHelper::new();

            helper
                .chain(&[None, Some("A"), Some("B"), Some("C"), Some("D")])
                .unwrap();
            helper
                .chain(&[Some("B"), Some("E"), Some("F"), Some("G"), Some("H")])
                .unwrap();
            helper.chain(&[Some("G"), Some("J"), Some("K")]).unwrap();
            helper
                .chain(&[Some("C"), Some("L"), Some("M"), Some("N")])
                .unwrap();

            helper
        }

        #[rstest]
        fn find_the_nearest_fork_point(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("H", "K")?, "G");

            Ok(())
        }

        #[rstest]
        fn find_an_ancestor_multiple_forks_away(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("D", "K")?, "B");

            Ok(())
        }

        #[rstest]
        fn find_the_same_fork_point_for_any_point_on_a_branch(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("D", "L")?, "C");
            assert_eq!(helper.ancestor("M", "D")?, "C");
            assert_eq!(helper.ancestor("D", "N")?, "C");

            Ok(())
        }

        #[rstest]
        fn find_the_commit_that_is_an_ancestor_of_the_other(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("K", "E")?, "E");

            Ok(())
        }

        #[rstest]
        fn find_a_root_commit(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("J", "A")?, "A");

            Ok(())
        }
    }
}
