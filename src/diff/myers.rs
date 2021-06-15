use crate::diff::{Edit, EditType, Line};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct Myers {
    a: Vec<Line>,
    b: Vec<Line>,
}

impl Myers {
    pub fn new(a: Vec<Line>, b: Vec<Line>) -> Self {
        Myers { a, b }
    }

    pub fn diff(&self) -> Vec<Edit> {
        let mut diff = vec![];

        for (prev_x, prev_y, x, y) in self.backtrack() {
            // TODO: Why does this happen?
            let a_line = if (prev_x as usize) < self.a.len() {
                Some(self.a[prev_x as usize].clone())
            } else {
                None
            };
            let b_line = if (prev_y as usize) < self.b.len() {
                Some(self.b[prev_y as usize].clone())
            } else {
                None
            };

            if x == prev_x {
                diff.push(Edit::new(EditType::Ins, None, b_line));
            } else if y == prev_y {
                diff.push(Edit::new(EditType::Del, a_line, None));
            } else {
                diff.push(Edit::new(EditType::Eql, a_line, b_line));
            }
        }

        diff.reverse();
        diff
    }

    fn backtrack(&self) -> Vec<(isize, isize, isize, isize)> {
        let mut x = self.a.len() as isize;
        let mut y = self.b.len() as isize;
        let mut result = vec![];

        for (d, v) in self.shortest_edit().iter().enumerate().rev() {
            let d = d as isize;
            let k = x - y;

            let prev_k = if k == -d || (k != d && v[&(k - 1)] < v[&(k + 1)]) {
                k + 1
            } else {
                k - 1
            };

            let prev_x = v[&prev_k];
            let prev_y = prev_x - prev_k;

            while x > prev_x && y > prev_y {
                result.push((x - 1, y - 1, x, y));
                x -= 1;
                y -= 1;
            }

            if d > 0 {
                result.push((prev_x, prev_y, x, y));
            }

            x = prev_x;
            y = prev_y;
        }

        result
    }

    #[allow(clippy::many_single_char_names)]
    fn shortest_edit(&self) -> Vec<BTreeMap<isize, isize>> {
        let n = self.a.len() as isize;
        let m = self.b.len() as isize;
        let max = n + m;

        let mut v = BTreeMap::new();
        v.insert(1_isize, 0);
        let mut trace = vec![];

        for d in 0..=max {
            trace.push(v.clone());

            for k in (-d..=d).step_by(2) {
                let mut x = if k == -d || (k != d && v[&(k - 1)] < v[&(k + 1)]) {
                    v[&(k + 1)]
                } else {
                    v[&(k - 1)] + 1
                };

                let mut y = x - k;

                while x < n && y < m && self.a[x as usize].text == self.b[y as usize].text {
                    x += 1;
                    y += 1;
                }

                v.insert(k, x);

                if x >= n && y >= m {
                    return trace;
                }
            }
        }
        unreachable!();
    }
}
