use crate::database::ParsedObject;
use crate::diff::hunk::Hunk;
use crate::diff::{diff_hunks, Edit, EditType};
use crate::repository::ChangeType;
use colored::Colorize;
use std::cell::RefMut;
use std::io::Write;
pub struct Diff<O: Write, E: Write> {
    ctx: CommandContext<O, E>,
impl<O: Write, E: Write> Diff<O, E> {
    pub fn new(ctx: CommandContext<O, E>) -> Self {
        Self { ctx }
        self.ctx.repo.index.load()?;
        self.ctx.repo.initialize_status()?;
        if self.ctx.argv.contains(&String::from("--cached")) {
    fn diff_head_index(&mut self) -> Result<()> {
        let paths: Vec<_> = self.ctx.repo.index_changes.keys().cloned().collect();
        for path in paths {
            let state = &self.ctx.repo.index_changes[&path];
                    let mut a = self.from_nothing(&path);
                    let mut b = self.from_index(&path)?;

                    self.print_diff(&mut a, &mut b)?;
                    let mut a = self.from_head(&path)?;
                    let mut b = self.from_index(&path)?;

                    self.print_diff(&mut a, &mut b)?;
                    let mut a = self.from_head(&path)?;
                    let mut b = self.from_nothing(&path);

                    self.print_diff(&mut a, &mut b)?;
    fn diff_index_workspace(&mut self) -> Result<()> {
        let paths: Vec<_> = self.ctx.repo.workspace_changes.keys().cloned().collect();
        for path in paths {
            let state = &self.ctx.repo.workspace_changes[&path];
                    let mut a = self.from_index(&path)?;
                    let mut b = self.from_file(&path)?;

                    self.print_diff(&mut a, &mut b)?;
                    let mut a = self.from_index(&path)?;
                    let mut b = self.from_nothing(&path);

                    self.print_diff(&mut a, &mut b)?;
    fn from_head(&mut self, path: &str) -> Result<Target> {
        let entry = &self.ctx.repo.head_tree[path];
        let oid = entry.oid();
        let blob = match self.ctx.repo.database.load(&oid)? {
            ParsedObject::Blob(blob) => blob,
            _ => unreachable!(),
        };

        Ok(Target::new(
            path.to_string(),
            oid,
            Some(entry.mode()),
            blob.data.clone(),
        ))
    fn from_index(&mut self, path: &str) -> Result<Target> {
        let entry = self.ctx.repo.index.entry_for_path(path);
        let blob = match self.ctx.repo.database.load(&entry.oid)? {
            ParsedObject::Blob(blob) => blob,
            _ => unreachable!(),
        };

        Ok(Target::new(
            path.to_string(),
            entry.oid.clone(),
            Some(entry.mode),
            blob.data.clone(),
        ))
        let blob = Blob::new(self.ctx.repo.workspace.read_file(Path::new(path))?);
        let oid = self.ctx.repo.database.hash_object(&blob);
        let mode = Entry::mode_for_stat(&self.ctx.repo.stats[path]);
        Ok(Target::new(path.to_string(), oid, Some(mode), blob.data))
        Target::new(path.to_string(), NULL_OID.to_string(), None, vec![])
        self.ctx.repo.database.short_oid(oid)
    fn print_diff(&self, a: &mut Target, b: &mut Target) -> Result<()> {
            return Ok(());
        let mut stdout = self.ctx.stdout.borrow_mut();
        writeln!(stdout, "diff --git {} {}", a.path, b.path)?;
        self.print_diff_mode(&mut stdout, &a, &b)?;
        self.print_diff_content(&mut stdout, &a, &b)?;

        Ok(())
    }

    fn header(&self, stdout: &mut RefMut<O>, string: String) -> Result<()> {
        writeln!(stdout, "{}", string.bold())?;

        Ok(())
    fn print_diff_mode(&self, stdout: &mut RefMut<O>, a: &Target, b: &Target) -> Result<()> {
            self.header(stdout, format!("new file mode {:o}", b.mode.unwrap()))?;
            self.header(stdout, format!("deleted file mode {:o}", a.mode.unwrap()))?;
            self.header(stdout, format!("old mode {:o}", a.mode.unwrap()))?;
            self.header(stdout, format!("new mode {:o}", b.mode.unwrap()))?;

        Ok(())
    fn print_diff_content(&self, stdout: &mut RefMut<O>, a: &Target, b: &Target) -> Result<()> {
            return Ok(());
        writeln!(stdout, "{}", oid_range)?;
        writeln!(stdout, "--- {}", a.diff_path())?;
        writeln!(stdout, "+++ {}", b.diff_path())?;

        let hunks = diff_hunks(
            std::str::from_utf8(&a.data).expect("Invalid UTF-8"),
            std::str::from_utf8(&b.data).expect("Invalid UTF-8"),
        );
        for hunk in hunks {
            self.print_diff_hunk(stdout, &hunk)?;
        }

        Ok(())
    }

    fn print_diff_hunk(&self, stdout: &mut RefMut<O>, hunk: &Hunk) -> Result<()> {
        writeln!(stdout, "{}", hunk.header().cyan())?;
        for edit in &hunk.edits {
            self.print_diff_edit(stdout, &edit)?;
        }

        Ok(())
    }

    fn print_diff_edit(&self, stdout: &mut RefMut<O>, edit: &Edit) -> Result<()> {
        let text = edit.to_string();

        match edit.r#type {
            EditType::Eql => writeln!(stdout, "{}", text)?,
            EditType::Ins => writeln!(stdout, "{}", text.green())?,
            EditType::Del => writeln!(stdout, "{}", text.red())?,
        }

        Ok(())
    data: Vec<u8>,
    fn new(path: String, oid: String, mode: Option<u32>, data: Vec<u8>) -> Self {
        Target {
            path,
            oid,
            mode,
            data,
        }