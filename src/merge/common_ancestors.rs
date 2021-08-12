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
    Result,
    Stale,
}

#[derive(Debug)]
pub struct CommonAncestors<'a> {
    database: &'a Database,
    flags: HashMap<String, HashSet<Flag>>,
    queue: VecDeque<Commit>,
    results: VecDeque<Commit>,
}

impl<'a> CommonAncestors<'a> {
    pub fn new(database: &'a Database, one: &str, twos: &[&str]) -> Result<Self> {
        let mut queue = VecDeque::new();
        let mut flags = HashMap::new();

        Self::insert_by_date(&mut queue, database.load_commit(one)?);
        let mut one_flags = HashSet::new();
        one_flags.insert(Flag::Parent1);
        flags.insert(one.to_string(), one_flags);

        for two in twos {
            Self::insert_by_date(&mut queue, database.load_commit(two)?);
            // Use `flags.entry(two)` to grab the existing set of flags if `one == two`.
            let two_flags = flags.entry(two.to_string()).or_insert_with(HashSet::new);
            two_flags.insert(Flag::Parent2);
        }

        Ok(Self {
            database,
            flags,
            queue,
            results: VecDeque::new(),
        })
    }

    pub fn find(&mut self) -> Result<Vec<String>> {
        while !self.all_stale() {
            self.process_queue()?;
        }

        Ok(self
            .results
            .iter()
            .filter_map(|commit| {
                if !self.is_marked(commit.oid(), Flag::Stale) {
                    Some(commit.oid())
                } else {
                    None
                }
            })
            .collect())
    }

    pub fn is_marked(&self, oid: String, flag: Flag) -> bool {
        self.flags[&oid].contains(&flag)
    }

    fn all_stale(&self) -> bool {
        self.queue
            .iter()
            .all(|commit| self.is_marked(commit.oid(), Flag::Stale))
    }

    fn process_queue(&mut self) -> Result<()> {
        let commit = self.queue.pop_front().unwrap();
        let flags = self.flags.get_mut(&commit.oid()).unwrap();

        if flags == &*BOTH_PARENTS {
            flags.insert(Flag::Result);
            Self::insert_by_date(&mut self.results, commit.clone());
            // Add `flags` and `Flag::Stale` to the parents
            let mut flags = flags.clone();
            flags.insert(Flag::Stale);
            self.add_parents(&commit, &flags)?;
        } else {
            let flags = flags.clone();
            self.add_parents(&commit, &flags)?;
        }

        Ok(())
    }

    fn add_parents(&mut self, commit: &Commit, flags: &HashSet<Flag>) -> Result<()> {
        for parent in &commit.parents {
            let parent = self.database.load_commit(parent)?;

            let current_flags = self.flags.entry(parent.oid()).or_insert_with(HashSet::new);
            if current_flags.is_superset(flags) {
                continue;
            }

            for flag in flags {
                current_flags.insert(flag.to_owned());
            }
            Self::insert_by_date(&mut self.queue, parent);
        }

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
    use crate::merge::bases::Bases;
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

        pub fn commit(&mut self, parents: &[&str], message: &str) -> Result<()> {
            let author = Author::new(
                String::from("A. U. Thor"),
                String::from("author@example.com"),
                self.time,
            );
            let parents = parents
                .iter()
                .map(|parent| self.commits.get(parent.to_owned()).unwrap().to_owned())
                .collect();
            let commit = Commit::new(
                parents,
                "0".repeat(40),
                // author
                author.clone(),
                // committer
                author,
                message.to_string(),
            );

            self.database.store(&commit)?;
            self.commits.insert(message.to_string(), commit.oid());

            Ok(())
        }

        pub fn chain(&mut self, names: &[Option<&str>]) -> Result<()> {
            for window in names.windows(2) {
                let parents = if let Some(parent) = window[0] {
                    vec![parent]
                } else {
                    vec![]
                };
                let message = window[1].unwrap();

                self.commit(&parents, message)?;
            }

            Ok(())
        }

        pub fn ancestor(&self, left: &str, right: &str) -> Result<Vec<String>> {
            let mut common =
                CommonAncestors::new(&self.database, &self.commits[left], &[&self.commits[right]])?;

            Ok(common
                .find()?
                .iter()
                .map(|oid| self.database.load_commit(oid).unwrap().message)
                .collect())
        }

        pub fn merge_base(&self, left: &str, right: &str) -> Result<String> {
            let mut bases = Bases::new(&self.database, &self.commits[left], &self.commits[right])?;

            let result: Vec<_> = bases
                .find()?
                .iter()
                .map(|oid| self.database.load_commit(oid).unwrap().message)
                .collect();
            assert_eq!(result.len(), 1);

            Ok(result[0].clone())
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
            assert_eq!(helper.ancestor("D", "D")?, ["D"]);

            Ok(())
        }

        #[rstest]
        fn find_the_commit_that_is_an_ancestor_of_the_other(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("B", "D")?, ["B"]);

            Ok(())
        }

        #[rstest]
        fn find_the_same_commit_if_the_arguments_are_reversed(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("D", "B")?, ["B"]);

            Ok(())
        }

        #[rstest]
        fn find_a_root_commit(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("A", "C")?, ["A"]);

            Ok(())
        }

        #[rstest]
        fn find_the_intersection_of_a_root_commit_with_itself(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("A", "A")?, ["A"]);

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
            assert_eq!(helper.ancestor("H", "K")?, ["G"]);

            Ok(())
        }

        #[rstest]
        fn find_an_ancestor_multiple_forks_away(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("D", "K")?, ["B"]);

            Ok(())
        }

        #[rstest]
        fn find_the_same_fork_point_for_any_point_on_a_branch(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("D", "L")?, ["C"]);
            assert_eq!(helper.ancestor("M", "D")?, ["C"]);
            assert_eq!(helper.ancestor("D", "N")?, ["C"]);

            Ok(())
        }

        #[rstest]
        fn find_the_commit_that_is_an_ancestor_of_the_other(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("K", "E")?, ["E"]);

            Ok(())
        }

        #[rstest]
        fn find_a_root_commit(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("J", "A")?, ["A"]);

            Ok(())
        }
    }

    ///   A   B   C   G   H
    ///   o---o---o---o---o
    ///        \     /
    ///         o---o---o
    ///         D   E   F
    mod with_a_merge {
        use super::*;

        #[fixture]
        fn helper() -> GraphHelper {
            let mut helper = GraphHelper::new();

            helper
                .chain(&[None, Some("A"), Some("B"), Some("C")])
                .unwrap();
            helper
                .chain(&[Some("B"), Some("D"), Some("E"), Some("F")])
                .unwrap();
            helper.commit(&["C", "E"], "G").unwrap();
            helper.chain(&[Some("G"), Some("H")]).unwrap();

            helper
        }

        #[rstest]
        fn find_the_most_recent_common_ancestor(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("H", "F")?, ["E"]);

            Ok(())
        }

        #[rstest]
        fn find_the_common_ancestor_of_a_merge_and_its_parents(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("C", "G")?, ["C"]);
            assert_eq!(helper.ancestor("G", "E")?, ["E"]);

            Ok(())
        }
    }

    ///   A   B   C   G   H   J
    ///   o---o---o---o---o---o
    ///        \     /
    ///         o---o---o
    ///         D   E   F
    mod with_a_merge_further_from_one_parent {
        use super::*;

        #[fixture]
        fn helper() -> GraphHelper {
            let mut helper = GraphHelper::new();

            helper
                .chain(&[None, Some("A"), Some("B"), Some("C")])
                .unwrap();
            helper
                .chain(&[Some("B"), Some("D"), Some("E"), Some("F")])
                .unwrap();
            helper.commit(&["C", "E"], "G").unwrap();
            helper.chain(&[Some("G"), Some("H"), Some("J")]).unwrap();

            helper
        }

        #[rstest]
        fn find_all_the_common_ancestors(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("J", "F")?, &["E", "B"]);

            Ok(())
        }

        #[rstest]
        fn find_the_best_common_ancestor(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.merge_base("J", "F")?, "E");

            Ok(())
        }
    }

    ///   A   B   C       H   J
    ///   o---o---o-------o---o
    ///        \         /
    ///         o---o---o G
    ///         D  E \
    ///               o F
    mod with_commits_between_the_common_ancestor_and_the_merge {
        use super::*;

        #[fixture]
        fn helper() -> GraphHelper {
            let mut helper = GraphHelper::new();

            helper
                .chain(&[None, Some("A"), Some("B"), Some("C")])
                .unwrap();
            helper
                .chain(&[Some("B"), Some("D"), Some("E"), Some("F")])
                .unwrap();
            helper.chain(&[Some("E"), Some("G")]).unwrap();
            helper.commit(&["C", "G"], "H").unwrap();
            helper.chain(&[Some("H"), Some("J")]).unwrap();

            helper
        }

        #[rstest]
        fn find_all_the_common_ancestors(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("J", "F")?, ["B", "E"]);

            Ok(())
        }

        #[rstest]
        fn find_the_best_common_ancestor(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.merge_base("J", "F")?, "E");

            Ok(())
        }
    }

    ///   A   B   C             H   J
    ///   o---o---o-------------o---o
    ///        \      E        /
    ///         o-----o-------o
    ///        D \     \     / G
    ///           \     o   /
    ///            \    F  /
    ///             o-----o
    ///             P     Q
    mod with_enough_history_to_find_all_stale_results {
        use super::*;

        #[fixture]
        fn helper() -> GraphHelper {
            let mut helper = GraphHelper::new();

            helper
                .chain(&[None, Some("A"), Some("B"), Some("C")])
                .unwrap();
            helper
                .chain(&[Some("B"), Some("D"), Some("E"), Some("F")])
                .unwrap();
            helper.chain(&[Some("D"), Some("P"), Some("Q")]).unwrap();
            helper.commit(&["E", "Q"], "G").unwrap();
            helper.commit(&["C", "G"], "H").unwrap();
            helper.chain(&[Some("H"), Some("J")]).unwrap();

            helper
        }

        #[rstest]
        fn find_the_best_common_ancestor(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("J", "F")?, ["E"]);
            assert_eq!(helper.ancestor("F", "J")?, ["E"]);

            Ok(())
        }
    }

    ///         L   M   N   P   Q   R   S   T
    ///         o---o---o---o---o---o---o---o
    ///        /       /       /       /
    ///   o---o---o...o---o...o---o---o---o---o
    ///   A   B  C \  D  E \  F  G \  H   J   K
    ///             \       \       \
    ///              o---o---o---o---o---o
    ///              U   V   W   X   Y   Z
    mod with_many_common_ancestors {
        use super::*;

        #[fixture]
        fn helper() -> GraphHelper {
            let mut helper = GraphHelper::new();

            helper
                .chain(&[
                    None,
                    Some("A"),
                    Some("B"),
                    Some("C"),
                    Some("pad-1-1"),
                    Some("pad-1-2"),
                    Some("pad-1-3"),
                    Some("pad-1-4"),
                    Some("D"),
                    Some("E"),
                    Some("pad-2-1"),
                    Some("pad-2-2"),
                    Some("pad-2-3"),
                    Some("pad-2-4"),
                    Some("F"),
                    Some("G"),
                    Some("H"),
                    Some("J"),
                    Some("K"),
                ])
                .unwrap();

            helper.chain(&[Some("B"), Some("L"), Some("M")]).unwrap();
            helper.commit(&["M", "D"], "N").unwrap();
            helper.chain(&[Some("N"), Some("P")]).unwrap();
            helper.commit(&["P", "F"], "Q").unwrap();
            helper.chain(&[Some("Q"), Some("R")]).unwrap();
            helper.commit(&["R", "H"], "S").unwrap();
            helper.chain(&[Some("S"), Some("T")]).unwrap();

            helper.chain(&[Some("C"), Some("U"), Some("V")]).unwrap();
            helper.commit(&["V", "E"], "W").unwrap();
            helper.chain(&[Some("W"), Some("X")]).unwrap();
            helper.commit(&["X", "G"], "Y").unwrap();
            helper.chain(&[Some("Y"), Some("Z")]).unwrap();

            helper
        }

        #[rstest]
        fn find_multiple_candidate_common_ancestors(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.ancestor("T", "Z")?, &["G", "D", "B"]);

            Ok(())
        }

        #[rstest]
        fn find_the_best_common_ancestor(helper: GraphHelper) -> Result<()> {
            assert_eq!(helper.merge_base("T", "Z")?, "G");

            Ok(())
        }
    }
}
