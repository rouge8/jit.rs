use crate::diff::{Edit, EditType};

const HUNK_CONTEXT: isize = 3;

#[derive(Debug)]
pub struct Hunk {
    a_start: usize,
    b_start: usize,
    pub edits: Vec<Edit>,
}

#[derive(Debug)]
enum LineType {
    A,
    B,
}

impl Hunk {
    pub fn new(a_start: usize, b_start: usize, edits: Vec<Edit>) -> Self {
        Hunk {
            a_start,
            b_start,
            edits,
        }
    }

    pub fn filter(edits: Vec<Edit>) -> Vec<Hunk> {
        let mut hunks = vec![];
        let mut offset: isize = 0;

        loop {
            while offset < edits.len() as isize && edits[offset as usize].r#type == EditType::Eql {
                offset += 1;
            }
            if offset >= edits.len() as isize {
                return hunks;
            }

            offset -= HUNK_CONTEXT + 1;

            let a_start = if offset < 0 {
                0
            } else {
                edits[offset as usize].a_line.as_ref().unwrap().number
            };
            let b_start = if offset < 0 {
                0
            } else {
                edits[offset as usize].b_line.as_ref().unwrap().number
            };

            let mut hunk = Hunk::new(a_start, b_start, vec![]);
            offset = Hunk::build(&mut hunk, &edits, offset);
            hunks.push(hunk);
        }
    }

    pub fn header(&self) -> String {
        let (a_start, a_lines) = self.offsets_for(LineType::A, self.a_start);
        let (b_start, b_lines) = self.offsets_for(LineType::B, self.b_start);

        format!("@@ -{},{} +{},{} @@", a_start, a_lines, b_start, b_lines)
    }

    fn build(hunk: &mut Hunk, edits: &[Edit], offset: isize) -> isize {
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
                match edits[(offset + HUNK_CONTEXT) as usize].r#type {
                    EditType::Ins | EditType::Del => {
                        counter = 2 * HUNK_CONTEXT + 1;
                    }
                    _ => {
                        counter -= 1;
                    }
                }
            }
        }

        offset
    }

    fn offsets_for(&self, line_type: LineType, default: usize) -> (usize, usize) {
        let lines: Vec<_> = self
            .edits
            .iter()
            .map(|edit| match line_type {
                LineType::A => &edit.a_line,
                LineType::B => &edit.b_line,
            })
            .filter_map(|line| line.as_ref())
            .collect();

        let start = if !lines.is_empty() {
            lines[0].number
        } else {
            default
        };

        (start, lines.len())
    }
}
