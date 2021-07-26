use crate::diff::{diff, EditType};
use crate::util::LinesWithEndings;
use std::collections::HashMap;

pub fn merge(o: &str, a: &str, b: &str) -> Result {
    let o: Vec<_> = LinesWithEndings::from(o).map(|l| l.to_string()).collect();
    let a: Vec<_> = LinesWithEndings::from(a).map(|l| l.to_string()).collect();
    let b: Vec<_> = LinesWithEndings::from(b).map(|l| l.to_string()).collect();

    Diff3::new(o, a, b).merge()
}

type MatchSet = HashMap<usize, usize>;

#[derive(Debug)]
struct Diff3 {
    o: Vec<String>,
    a: Vec<String>,
    b: Vec<String>,
    chunks: Vec<Chunk>,
    line_o: usize,
    line_a: usize,
    line_b: usize,
    match_a: MatchSet,
    match_b: MatchSet,
}

impl Diff3 {
    pub fn new(o: Vec<String>, a: Vec<String>, b: Vec<String>) -> Self {
        Self {
            o,
            a,
            b,
            chunks: Vec::new(),
            line_o: 0,
            line_a: 0,
            line_b: 0,
            match_a: HashMap::new(),
            match_b: HashMap::new(),
        }
    }

    pub fn merge(&mut self) -> Result {
        self.setup();
        self.generate_chunks();
        Result::new(self.chunks.clone())
    }

    fn setup(&mut self) {
        self.chunks = Vec::new();
        self.line_o = 0;
        self.line_a = 0;
        self.line_b = 0;

        self.match_a = self.match_set(&self.a);
        self.match_b = self.match_set(&self.b);
    }

    fn match_set(&self, file: &[String]) -> MatchSet {
        let mut matches = HashMap::new();

        for edit in diff(&self.o.join("\n"), &file.join("\n")) {
            match edit.r#type {
                EditType::Eql => {
                    matches.insert(edit.a_line.unwrap().number, edit.b_line.unwrap().number);
                }
                _ => continue,
            }
        }

        matches
    }

    #[allow(clippy::unnecessary_unwrap)]
    fn generate_chunks(&mut self) {
        loop {
            let i = self.find_next_mismatch();

            if let Some(i) = i {
                if i == 1 {
                    let (o, a, b) = self.find_next_match();

                    if a.is_some() && b.is_some() {
                        self.emit_chunk(o, a.unwrap(), b.unwrap());
                    } else {
                        self.emit_final_chunk();
                        return;
                    }
                } else {
                    self.emit_chunk(self.line_o + i, self.line_a + i, self.line_b + i);
                }
            } else {
                self.emit_final_chunk();
                return;
            }
        }
    }

    fn find_next_mismatch(&self) -> Option<usize> {
        let mut i = 1;

        while self.in_bounds(i)
            && self.matches(&self.match_a, self.line_a, i)
            && self.matches(&self.match_b, self.line_b, i)
        {
            i += 1;
        }

        if self.in_bounds(i) {
            Some(i)
        } else {
            None
        }
    }

    fn in_bounds(&self, i: usize) -> bool {
        self.line_o + i <= self.o.len()
            || self.line_a + i <= self.a.len()
            || self.line_b + i <= self.b.len()
    }

    fn matches(&self, matches: &MatchSet, offset: usize, i: usize) -> bool {
        matches.get(&(self.line_o + i)) == Some(&(offset + i))
    }

    fn find_next_match(&self) -> (usize, Option<usize>, Option<usize>) {
        let mut o = self.line_o + 1;

        while o <= self.o.len() && !(self.match_a.contains_key(&o) && self.match_b.contains_key(&o))
        {
            o += 1;
        }

        (
            o,
            self.match_a.get(&o).copied(),
            self.match_b.get(&o).copied(),
        )
    }

    fn emit_chunk(&mut self, o: usize, a: usize, b: usize) {
        let self_o = self.o.clone();
        let self_a = self.a.clone();
        let self_b = self.b.clone();

        self.write_chunk(
            &self_o[self.line_o..o - 1],
            &self_a[self.line_a..a - 1],
            &self_b[self.line_b..b - 1],
        );

        self.line_o = o - 1;
        self.line_a = a - 1;
        self.line_b = b - 1;
    }

    fn emit_final_chunk(&mut self) {
        let self_o = self.o.clone();
        let self_a = self.a.clone();
        let self_b = self.b.clone();

        self.write_chunk(
            &self_o[self.line_o..],
            &self_a[self.line_a..],
            &self_b[self.line_b..],
        );
    }

    fn write_chunk(&mut self, o: &[String], a: &[String], b: &[String]) {
        if a == o || a == b {
            self.chunks.push(Chunk::Clean { lines: b.to_vec() });
        } else if b == o {
            self.chunks.push(Chunk::Clean { lines: a.to_vec() });
        } else {
            self.chunks.push(Chunk::Conflict {
                o_lines: o.to_vec(),
                a_lines: a.to_vec(),
                b_lines: b.to_vec(),
            });
        }
    }
}

#[derive(Debug, Clone)]
pub enum Chunk {
    Clean {
        lines: Vec<String>,
    },
    Conflict {
        o_lines: Vec<String>,
        a_lines: Vec<String>,
        b_lines: Vec<String>,
    },
}

impl Chunk {
    pub fn to_string(&self, a_name: Option<&str>, b_name: Option<&str>) -> String {
        match self {
            Chunk::Clean { lines } => lines.join(""),
            Chunk::Conflict {
                o_lines: _,
                a_lines,
                b_lines,
            } => {
                fn separator(text: &mut String, r#char: &str, name: Option<&str>) {
                    text.push_str(&r#char.repeat(7));
                    if let Some(name) = name {
                        text.push_str(&format!(" {}", name));
                    }
                    text.push('\n');
                }

                let mut text = String::new();
                separator(&mut text, "<", a_name);
                for line in a_lines {
                    text.push_str(&line);
                }
                separator(&mut text, "=", None);
                for line in b_lines {
                    text.push_str(&line);
                }
                separator(&mut text, ">", b_name);

                text
            }
        }
    }
}

#[derive(Debug)]
pub struct Result {
    chunks: Vec<Chunk>,
}

impl Result {
    pub fn new(chunks: Vec<Chunk>) -> Self {
        Self { chunks }
    }

    pub fn is_clean(&self) -> bool {
        for chunk in &self.chunks {
            match chunk {
                Chunk::Clean { .. } => continue,
                Chunk::Conflict { .. } => return false,
            }
        }

        true
    }

    pub fn to_string(&self, a_name: Option<&str>, b_name: Option<&str>) -> String {
        self.chunks
            .iter()
            .map(|chunk| chunk.to_string(a_name, b_name))
            .collect::<Vec<_>>()
            .join("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanly_merge_two_lists() {
        let merge = merge(
            "\
a
b
c", "\
d
b
c", "\
a
b
e",
        );

        assert!(merge.is_clean());
        assert_eq!(
            merge.to_string(None, None),
            "\
d
b
e"
        );
    }

    #[test]
    fn cleanly_merge_two_lists_with_the_same_edit() {
        let merge = merge(
            "\
a
b
c", "\
d
b
c", "\
d
b
e",
        );

        assert!(merge.is_clean());
        assert_eq!(
            merge.to_string(None, None),
            "\
d
b
e"
        );
    }

    #[test]
    fn uncleanly_merge_two_lists() {
        let merge = merge(
            "\
a
b
c", "\
d
b
c", "\
e
b
c",
        );

        assert!(!merge.is_clean());
        assert_eq!(
            merge.to_string(None, None),
            "\
<<<<<<<
d
=======
e
>>>>>>>
b
c"
        );
    }

    #[test]
    fn uncleanly_merge_two_lists_against_an_empty_list() {
        let merge = merge(
            "", "\
d
b
c", "\
e
b
c",
        );

        assert!(!merge.is_clean());
        assert_eq!(
            merge.to_string(None, None),
            "\
<<<<<<<
d
b
c=======
e
b
c>>>>>>>
"
        );
    }

    #[test]
    fn uncleanly_merge_two_lists_with_head_names() {
        let merge = merge(
            "\
a
b
c", "\
d
b
c", "\
e
b
c",
        );

        assert!(!merge.is_clean());
        assert_eq!(
            merge.to_string(Some("left"), Some("right")),
            "\
<<<<<<< left
d
=======
e
>>>>>>> right
b
c"
        );
    }
}
