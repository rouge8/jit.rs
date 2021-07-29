use crate::diff::{EditType, Line};
use crate::util::transpose;
use std::fmt;

const HUNK_CONTEXT: isize = 3;

pub trait GenericEdit: Clone + fmt::Display {
    fn r#type(&self) -> EditType;

    fn a_lines(&self) -> Vec<Option<Line>>;

    fn b_line(&self) -> Option<Line>;
}

#[derive(Debug)]
pub struct Hunk<T: GenericEdit> {
    a_starts: Vec<Option<usize>>,
    b_start: usize,
    pub edits: Vec<T>,
}

impl<T> Hunk<T>
where
    T: GenericEdit,
{
    pub fn new(a_starts: Vec<Option<usize>>, b_start: usize, edits: Vec<T>) -> Self {
        Hunk {
            a_starts,
            b_start,
            edits,
        }
    }

    pub fn filter(edits: Vec<T>) -> Vec<Hunk<T>> {
        let mut hunks = vec![];
        let mut offset: isize = 0;

        loop {
            while offset < edits.len() as isize && edits[offset as usize].r#type() == EditType::Eql
            {
                offset += 1;
            }
            if offset >= edits.len() as isize {
                return hunks;
            }

            offset -= HUNK_CONTEXT + 1;

            let a_starts = if offset < 0 {
                vec![]
            } else {
                edits[offset as usize]
                    .a_lines()
                    .iter()
                    .map(|line| line.as_ref().map(|line| line.number))
                    .collect()
            };
            let b_start = if offset < 0 {
                0
            } else {
                edits[offset as usize].b_line().as_ref().unwrap().number
            };

            let mut hunk = Hunk::new(a_starts, b_start, vec![]);
            offset = Hunk::build(&mut hunk, &edits, offset);
            hunks.push(hunk);
        }
    }

    pub fn header(&self) -> String {
        let a_lines = transpose(self.edits.iter().map(|edit| edit.a_lines()).collect());
        let mut offsets: Vec<_> = a_lines
            .iter()
            .enumerate()
            .map(|(i, lines)| {
                Self::format(
                    "-",
                    lines.to_vec(),
                    if let Some(start) = self.a_starts.get(i) {
                        *start
                    } else {
                        None
                    },
                )
            })
            .collect();

        let b_lines: Vec<_> = self.edits.iter().map(|edit| edit.b_line()).collect();
        offsets.push(Self::format("+", b_lines, Some(self.b_start)));

        let sep = "@".repeat(offsets.len());
        let mut result = vec![sep.clone()];
        result.append(&mut offsets);
        result.push(sep);

        result.join(" ")
    }

    fn format(sign: &str, lines: Vec<Option<Line>>, start: Option<usize>) -> String {
        let lines: Vec<_> = lines.iter().filter_map(|line| line.as_ref()).collect();
        let start = if let Some(line) = lines.first() {
            line.number
        } else if let Some(start) = start {
            start
        } else {
            0
        };

        format!("{}{},{}", sign, start, lines.len())
    }

    fn build(hunk: &mut Hunk<T>, edits: &[T], offset: isize) -> isize {
        let mut counter = -1;
        let mut offset = offset;

        while counter != 0 {
            if offset >= 0 && counter > 0 {
                hunk.edits.push(edits[offset as usize].clone());
            }

            offset += 1;
            if offset >= edits.len() as isize {
                break;
            }

            if offset + HUNK_CONTEXT < edits.len() as isize {
                match edits[(offset + HUNK_CONTEXT) as usize].r#type() {
                    EditType::Ins | EditType::Del => {
                        counter = 2 * HUNK_CONTEXT + 1;
                    }
                    _ => {
                        counter -= 1;
                    }
                }
            } else {
                counter -= 1
            }
        }

        offset
    }
}
