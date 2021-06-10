use myers::Myers;
use std::fmt;

mod myers;

fn lines(document: &str) -> Vec<Line> {
    let mut result = vec![];

    for line in document.lines() {
        result.push(Line::new(line));
    }

    result
}

pub fn diff(a: &str, b: &str) -> Vec<Edit> {
    Myers::new(lines(a), lines(b)).diff()
}

#[derive(Debug, Clone)]
pub struct Line {
    text: String,
}

impl Line {
    pub fn new(text: &str) -> Self {
        Line {
            text: text.to_string(),
        }
    }
}

#[derive(Debug)]
pub struct Edit {
    r#type: EditType,
    text: String,
}

impl Edit {
    fn new(r#type: EditType, line: Line) -> Self {
        Edit {
            r#type,
            text: line.text,
        }
    }
}

impl fmt::Display for Edit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.r#type, self.text)
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
