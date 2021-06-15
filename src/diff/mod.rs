use myers::Myers;
use std::fmt;

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

#[derive(Debug, Clone)]
pub struct Line {
    number: usize,
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

#[derive(Debug)]
pub struct Edit {
    r#type: EditType,
    a_line: Option<Line>,
    b_line: Option<Line>,
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

#[derive(Debug)]
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
}
