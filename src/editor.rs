use crate::errors::{Error, Result};
use crate::util::{path_to_string, LinesWithEndings};
use regex::Regex;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::process::Command;

const DEFAULT_EDITOR: &str = "vi";

#[derive(Debug)]
pub struct Editor {
    path: PathBuf,
    command: String,
    closed: bool,
    file: File,
}

impl Editor {
    pub fn new(path: PathBuf, command: Option<String>) -> Result<Self> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;

        Ok(Self {
            path,
            command: command.unwrap_or_else(|| DEFAULT_EDITOR.to_owned()),
            closed: false,
            file,
        })
    }

    pub fn edit<F>(path: PathBuf, command: Option<String>, f: F) -> Result<Option<String>>
    where
        F: Fn(&mut Editor) -> Result<()>,
    {
        let mut editor = Editor::new(path, command)?;
        f(&mut editor)?;
        editor.edit_file()
    }

    pub fn write(&mut self, string: &str) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        self.file.write_all(string.as_bytes())?;
        self.file.write_all(b"\n")?;

        Ok(())
    }

    pub fn note(&mut self, string: &str) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        for line in LinesWithEndings::from(string) {
            write!(self.file, "# {}", line)?;
        }

        Ok(())
    }

    pub fn close(&mut self) {
        self.closed = true;
    }

    pub fn edit_file(&mut self) -> Result<Option<String>> {
        let fd = self.file.as_raw_fd();
        unsafe {
            libc::close(fd);
        }

        let mut editor_argv = shlex::split(&self.command).expect("Invalid command");
        editor_argv.push(path_to_string(&self.path));
        let cmd = editor_argv[0].clone();
        editor_argv.remove(0);

        if !self.closed {
            let status = Command::new(cmd).args(&editor_argv).status()?;
            if !status.success() {
                return Err(Error::ProblemWithEditor(self.command.clone()));
            }
        }

        Ok(self.remove_notes(fs::read_to_string(&self.path)?))
    }

    fn remove_notes(&self, string: String) -> Option<String> {
        let lines: Vec<_> = LinesWithEndings::from(&string)
            .filter(|line| !line.starts_with('#'))
            .collect();

        let re = Regex::new(r"^\s*$").unwrap();
        if lines.iter().all(|line| re.is_match(line)) {
            None
        } else {
            Some(format!("{}\n", lines.join("").trim()))
        }
    }
}
