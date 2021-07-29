use crate::diff::hunk::GenericEdit;
use crate::diff::{Edit, EditType, Line};
use std::fmt;

pub struct Combined {
    diffs: Vec<Vec<Edit>>,
    offsets: Vec<usize>,
    diffs_index: usize,
}

impl Combined {
    pub fn new(diffs: Vec<Vec<Edit>>) -> Self {
        let offsets = diffs.iter().map(|_diff| 0).collect();

        Self {
            diffs,
            offsets,
            diffs_index: 0,
        }
    }

    fn is_complete(&self) -> bool {
        let mut offset_diffs = self.offsets.iter().zip(self.diffs.iter());

        offset_diffs.all(|(offset, diff)| *offset == diff.len())
    }

    fn consume_deletion(&mut self, diff: &[Edit], i: usize) -> Option<Row> {
        if self.offsets[i] < diff.len() && matches!(diff[self.offsets[i]].r#type, EditType::Del) {
            let mut edits: Vec<_> = self.diffs.iter().map(|_| None).collect();
            edits[i] = Some(diff[self.offsets[i]].clone());
            self.offsets[i] += 1;

            Some(Row::new(edits))
        } else {
            None
        }
    }
}

impl Iterator for Combined {
    type Item = Row;

    fn next(&mut self) -> Option<Self::Item> {
        if self.diffs_index < self.diffs.len() {
            let i = self.diffs_index;
            let diff = self.diffs[i].clone();

            if let Some(row) = self.consume_deletion(&diff, i) {
                return Some(row);
            } else {
                self.diffs_index += 1;
                return self.next();
            }
        }

        if self.is_complete() {
            return None;
        }

        let offset_diffs = self.offsets.iter().zip(self.diffs.iter());
        let edits: Vec<_> = offset_diffs
            .map(|(offset, diff)| diff.get(*offset).cloned())
            .collect();

        for i in 0..self.offsets.len() {
            self.offsets[i] += 1;
        }

        self.diffs_index = 0;

        Some(Row::new(edits))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    edits: Vec<Option<Edit>>,
}

impl Row {
    pub fn new(edits: Vec<Option<Edit>>) -> Self {
        Self { edits }
    }
}

impl fmt::Display for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let symbols: Vec<_> = self
            .edits
            .iter()
            .map(|edit| {
                if let Some(edit) = edit {
                    edit.r#type.to_string()
                } else {
                    String::from(" ")
                }
            })
            .collect();

        let del = self
            .edits
            .iter()
            .find(|edit| edit.is_some() && edit.as_ref().unwrap().r#type == EditType::Del);
        let line = if let Some(del) = del {
            del.as_ref().unwrap().a_line.as_ref().unwrap().text.clone()
        } else {
            self.edits[0]
                .as_ref()
                .unwrap()
                .b_line
                .as_ref()
                .unwrap()
                .text
                .clone()
        };
        write!(f, "{}{}", symbols.join(""), line)
    }
}

impl GenericEdit for Row {
    fn r#type(&self) -> EditType {
        let types: Vec<_> = self
            .edits
            .iter()
            .filter_map(|edit| edit.as_ref().map(|edit| edit.r#type.clone()))
            .collect();

        if types.iter().any(|r#type| matches!(r#type, EditType::Ins)) {
            EditType::Ins
        } else {
            types[0].clone()
        }
    }

    fn a_lines(&self) -> Vec<Option<Line>> {
        self.edits
            .iter()
            .map(|edit| {
                if let Some(edit) = edit {
                    edit.a_line.clone()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    }

    fn b_line(&self) -> Option<Line> {
        if let Some(edit) = &self.edits[0] {
            edit.b_line.clone()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::Line;

    #[test]
    fn it_works() {
        let left_diff = vec![
            Edit::new(EditType::Del, Some(Line::new(1, "alfa")), None),
            Edit::new(EditType::Ins, None, Some(Line::new(1, "echo"))),
            Edit::new(
                EditType::Eql,
                Some(Line::new(2, "bravo")),
                Some(Line::new(2, "bravo")),
            ),
            Edit::new(
                EditType::Eql,
                Some(Line::new(3, "delta")),
                Some(Line::new(3, "delta")),
            ),
            Edit::new(EditType::Ins, None, Some(Line::new(4, "foxtrot"))),
        ];
        let right_diff = vec![
            Edit::new(
                EditType::Eql,
                Some(Line::new(1, "echo")),
                Some(Line::new(1, "echo")),
            ),
            Edit::new(
                EditType::Eql,
                Some(Line::new(2, "bravo")),
                Some(Line::new(2, "bravo")),
            ),
            Edit::new(EditType::Del, Some(Line::new(3, "charlie")), None),
            Edit::new(EditType::Ins, None, Some(Line::new(3, "delta"))),
            Edit::new(EditType::Ins, None, Some(Line::new(4, "foxtrot"))),
        ];
        let expected = vec![
            Row::new(vec![
                Some(Edit::new(EditType::Del, Some(Line::new(1, "alfa")), None)),
                None,
            ]),
            Row::new(vec![
                Some(Edit::new(EditType::Ins, None, Some(Line::new(1, "echo")))),
                Some(Edit::new(
                    EditType::Eql,
                    Some(Line::new(1, "echo")),
                    Some(Line::new(1, "echo")),
                )),
            ]),
            Row::new(vec![
                Some(Edit::new(
                    EditType::Eql,
                    Some(Line::new(2, "bravo")),
                    Some(Line::new(2, "bravo")),
                )),
                Some(Edit::new(
                    EditType::Eql,
                    Some(Line::new(2, "bravo")),
                    Some(Line::new(2, "bravo")),
                )),
            ]),
            Row::new(vec![
                None,
                Some(Edit::new(
                    EditType::Del,
                    Some(Line::new(3, "charlie")),
                    None,
                )),
            ]),
            Row::new(vec![
                Some(Edit::new(
                    EditType::Eql,
                    Some(Line::new(3, "delta")),
                    Some(Line::new(3, "delta")),
                )),
                Some(Edit::new(EditType::Ins, None, Some(Line::new(3, "delta")))),
            ]),
            Row::new(vec![
                Some(Edit::new(
                    EditType::Ins,
                    None,
                    Some(Line::new(4, "foxtrot")),
                )),
                Some(Edit::new(
                    EditType::Ins,
                    None,
                    Some(Line::new(4, "foxtrot")),
                )),
            ]),
        ];

        assert_eq!(
            Combined::new(vec![left_diff, right_diff]).collect::<Vec<_>>(),
            expected
        );
    }
}
