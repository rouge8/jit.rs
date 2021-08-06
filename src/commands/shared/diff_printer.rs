use crate::diff::hunk::{GenericEdit, Hunk};
use crate::diff::{combined_hunks, diff_hunks, EditType};
#[derive(Debug, Clone)]
    pub fn from_entry(
        &self,
        repo: &Repository,
        path: &str,
        entry: Option<&Entry>,
    ) -> Result<Target> {
            None => Ok(self.from_nothing(path)),
            let path = path_to_string(path);
                &mut self.from_entry(repo, &path, old_entry.as_ref())?,
                &mut self.from_entry(repo, &path, new_entry.as_ref())?,
        self.print_diff_mode(stdout, a, b)?;
        self.print_diff_content(stdout, a, b)?;
    pub fn print_combined_diff(
        &self,
        stdout: &mut RefMut<Box<dyn Write>>,
        r#as: &[Target],
        b: &Target,
    ) -> Result<()> {
        self.header(stdout, format!("diff --cc {}", b.path))?;

        let a_oids: Vec<_> = r#as.iter().map(|a| self.short(&a.oid)).collect();
        let oid_range = format!("index {}..{}", a_oids.join(","), self.short(&b.oid));
        self.header(stdout, oid_range)?;

        if !r#as.iter().all(|a| a.mode == b.mode) {
            self.header(
                stdout,
                format!(
                    "mode {}..{:o}",
                    r#as.iter()
                        .map(|a| format!("{:o}", a.mode.unwrap()))
                        .collect::<Vec<_>>()
                        .join(","),
                    b.mode.unwrap()
                ),
            )?;
        }

        self.header(stdout, format!("--- a/{}", b.diff_path()))?;
        self.header(stdout, format!("+++ b/{}", b.diff_path()))?;

        let hunks = combined_hunks(
            &r#as
                .iter()
                .map(|a| std::str::from_utf8(&a.data).expect("Invalid UTF-8"))
                .collect::<Vec<_>>(),
            std::str::from_utf8(&b.data).expect("Invalid UTF-8"),
        );
        for hunk in hunks {
            self.print_diff_hunk(stdout, &hunk)?;
        }

        Ok(())
    }

    fn print_diff_hunk<T: GenericEdit>(
        &self,
        stdout: &mut RefMut<Box<dyn Write>>,
        hunk: &Hunk<T>,
    ) -> Result<()> {
            self.print_diff_edit(stdout, edit)?;
    fn print_diff_edit<T: GenericEdit>(
        &self,
        stdout: &mut RefMut<Box<dyn Write>>,
        edit: &T,
    ) -> Result<()> {
        match edit.r#type() {