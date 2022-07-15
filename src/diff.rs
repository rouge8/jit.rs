use std::fmt;

use combined::{Combined, Row};
use hunk::{GenericEdit, Hunk};
use myers::Myers;

mod combined;
pub mod hunk;
mod myers;

fn lines(document: &str) -> Vec<Line> {
    let mut result = vec![];

    for (i, line) in document.lines().enumerate() {
        result.push(Line::new(i + 1, line));
    }

    result
}

pub fn diff(a: &str, b: &str) -> Vec<Edit> {
    Myers::new(lines(a), lines(b)).diff()
}

pub fn diff_hunks(a: &str, b: &str) -> Vec<Hunk<Edit>> {
    Hunk::filter(diff(a, b))
}

pub fn combined(r#as: &[&str], b: &str) -> Vec<Row> {
    let diffs = r#as.iter().map(|a| diff(a, b)).collect();

    Combined::new(diffs).collect()
}

pub fn combined_hunks(r#as: &[&str], b: &str) -> Vec<Hunk<Row>> {
    Hunk::filter(combined(r#as, b))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub number: usize,
    text: String,
}

impl Line {
    pub fn new(number: usize, text: &str) -> Self {
        Line {
            number,
            text: text.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub r#type: EditType,
    pub a_line: Option<Line>,
    pub b_line: Option<Line>,
}

impl Edit {
    fn new(r#type: EditType, a_line: Option<Line>, b_line: Option<Line>) -> Self {
        Edit {
            r#type,
            a_line,
            b_line,
        }
    }
}

impl fmt::Display for Edit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let line = self
            .a_line
            .as_ref()
            .unwrap_or_else(|| self.b_line.as_ref().unwrap());
        write!(f, "{}{}", self.r#type, line.text)
    }
}

impl GenericEdit for Edit {
    fn r#type(&self) -> EditType {
        self.r#type.clone()
    }

    fn a_lines(&self) -> Vec<Option<Line>> {
        vec![self.a_line.clone()]
    }

    fn b_line(&self) -> Option<Line> {
        self.b_line.clone()
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum EditType {
    Eql,
    Ins,
    Del,
}

impl fmt::Display for EditType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let result = match self {
            EditType::Eql => " ",
            EditType::Ins => "+",
            EditType::Del => "-",
        };

        write!(f, "{}", result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_diffs() {
        let a = "\
A
B
C
A
B
B
A";
        let b = "\
C
B
A
B
A
C";

        let result = diff(a, b)
            .into_iter()
            .map(|edit| edit.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(
            result,
            "\
-A
-B
 C
+B
 A
 B
-B
 A
+C"
        );
    }

    mod diff_hunks {
        use super::*;

        const DOC: &str = "\
the
quick
brown
fox
jumps
over
the
lazy
dog";

        fn hunks(a: &str, b: &str) -> Vec<(String, Vec<String>)> {
            diff_hunks(a, b)
                .iter()
                .map(|hunk| {
                    (
                        hunk.header(),
                        hunk.edits.iter().map(|edit| edit.to_string()).collect(),
                    )
                })
                .collect()
        }

        #[test]
        fn detect_deletion_at_the_start() {
            let changed = "\
quick
brown
fox
jumps
over
the
lazy
dog";

            assert_eq!(
                hunks(DOC, changed),
                vec![(
                    String::from("@@ -1,4 +1,3 @@"),
                    vec![
                        String::from("-the"),
                        String::from(" quick"),
                        String::from(" brown"),
                        String::from(" fox")
                    ]
                )]
            );
        }

        #[test]
        fn detect_insertion_at_the_start() {
            let changed = "\
so
the
quick
brown
fox
jumps
over
the
lazy
dog";

            assert_eq!(
                hunks(DOC, changed),
                vec![(
                    String::from("@@ -1,3 +1,4 @@"),
                    vec![
                        String::from("+so"),
                        String::from(" the"),
                        String::from(" quick"),
                        String::from(" brown"),
                    ]
                )]
            );
        }

        #[test]
        fn detect_change_skipping_start_and_end() {
            let changed = "\
the
quick
brown
fox
leaps
right
over
the
lazy
dog";

            assert_eq!(
                hunks(DOC, changed),
                vec![(
                    String::from("@@ -2,7 +2,8 @@"),
                    vec![
                        String::from(" quick"),
                        String::from(" brown"),
                        String::from(" fox"),
                        String::from("-jumps"),
                        String::from("+leaps"),
                        String::from("+right"),
                        String::from(" over"),
                        String::from(" the"),
                        String::from(" lazy"),
                    ]
                )]
            );
        }

        #[test]
        fn put_nearby_changes_in_the_same_hunk() {
            let changed = "\
the
brown
fox
jumps
over
the
lazy
cat";

            assert_eq!(
                hunks(DOC, changed),
                vec![(
                    String::from("@@ -1,9 +1,8 @@"),
                    vec![
                        String::from(" the"),
                        String::from("-quick"),
                        String::from(" brown"),
                        String::from(" fox"),
                        String::from(" jumps"),
                        String::from(" over"),
                        String::from(" the"),
                        String::from(" lazy"),
                        String::from("-dog"),
                        String::from("+cat"),
                    ]
                )]
            );
        }

        #[test]
        fn put_distant_changes_in_different_hunks() {
            let changed = "\
a
quick
brown
fox
jumps
over
the
lazy
cat";

            assert_eq!(
                hunks(DOC, changed),
                vec![
                    (
                        String::from("@@ -1,4 +1,4 @@"),
                        vec![
                            String::from("-the"),
                            String::from("+a"),
                            String::from(" quick"),
                            String::from(" brown"),
                            String::from(" fox"),
                        ]
                    ),
                    (
                        String::from("@@ -6,4 +6,4 @@"),
                        vec![
                            String::from(" over"),
                            String::from(" the"),
                            String::from(" lazy"),
                            String::from("-dog"),
                            String::from("+cat"),
                        ]
                    ),
                ]
            );
        }
    }
}
