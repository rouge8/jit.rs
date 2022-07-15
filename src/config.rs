use std::fmt;
use std::fmt::Write;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use lazy_static::lazy_static;
use regex::{Regex, RegexBuilder};

use crate::errors::{Error, Result};
use crate::lockfile::Lockfile;

pub mod stack;

lazy_static! {
    static ref SECTION_LINE: Regex =
        // TODO: Handle difference between Ruby's \Z and Rust's \z
        RegexBuilder::new(r#"\A\s*\[([a-z0-9-]+)( "(.+)")?\]\s*(\z|#|;)"#)
            .case_insensitive(true)
            .build()
            .unwrap();

    static ref VARIABLE_LINE: Regex =
        // TODO: Handle difference between Ruby's \Z and Rust's \z
        RegexBuilder::new(r#"\A\s*([a-z][a-z0-9-]*)\s*=\s*(.*?)\s*(\z|#|;)"#)
            .case_insensitive(true)
            .multi_line(true)
            .build().unwrap();

    // TODO: Handle difference between Ruby's \Z and Rust's \z
    static ref BLANK_LINE: Regex = Regex::new(r#"\A\s*(\z|#|;)"#).unwrap();

    // TODO: Handle difference between Ruby's \Z and Rust's \z
    static ref INTEGER: Regex = Regex::new(r#"\A-?[1-9][0-9]*\z"#).unwrap();

    static ref VALID_SECTION: Regex = RegexBuilder::new(r"^[a-z0-9-]+$")
        .case_insensitive(true)
        .build()
        .unwrap();

    static ref VALID_VARIABLE: Regex = RegexBuilder::new(r"^[a-z][a-z0-9-]*$")
        .case_insensitive(true)
        .build()
        .unwrap();
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Variable {
    name: String,
    value: VariableValue,
}

impl Variable {
    pub fn new(name: String, value: VariableValue) -> Self {
        Self { name, value }
    }

    pub fn normalize(name: &str) -> String {
        name.to_lowercase()
    }

    pub fn serialize(name: &str, value: &VariableValue) -> String {
        format!("\t{} = {}\n", name, value)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum VariableValue {
    Bool(bool),
    Int(i32),
    String(String),
}

impl fmt::Display for VariableValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VariableValue::Bool(val) => write!(f, "{}", val),
            VariableValue::Int(val) => write!(f, "{}", val),
            VariableValue::String(val) => write!(f, "{}", val),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Section {
    name: Vec<String>,
}

impl Section {
    pub fn new(name: Vec<String>) -> Self {
        Self { name }
    }

    pub fn normalize(name: &[String]) -> Vec<String> {
        if let Some((first, rest)) = name.split_first() {
            vec![first.to_lowercase(), rest.join(".")]
        } else {
            vec![]
        }
    }

    pub fn heading_line(&self) -> String {
        let (first, rest) = self.name.split_first().unwrap();

        let mut line = format!("[{}", first);
        if !rest.is_empty() {
            write!(line, " \"{}\"", rest.join(".")).unwrap();
        }
        line.push_str("]\n");

        line
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Line {
    text: String,
    section: Section,
    variable: Option<Variable>,
}

impl Line {
    pub fn new(text: String, section: Section, variable: Option<Variable>) -> Self {
        Self {
            text,
            section,
            variable,
        }
    }

    fn normal_variable(&self) -> Option<String> {
        self.variable
            .as_ref()
            .map(|variable| Variable::normalize(&variable.name))
    }
}

#[derive(Debug)]
pub struct Config {
    path: PathBuf,
    lockfile: Lockfile,
    lines: IndexMap<Vec<String>, Vec<Line>>,
}

impl Config {
    pub fn is_valid_key(key: &[&str]) -> bool {
        let section = key.first().map_or_else(String::new, |key| key.to_string());
        let variable = key.last().map_or_else(String::new, |key| key.to_string());

        VALID_SECTION.is_match(&section) && VALID_VARIABLE.is_match(&variable)
    }

    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_owned(),
            lockfile: Lockfile::new(path.to_owned()),
            lines: IndexMap::new(),
        }
    }

    pub fn open(&mut self) -> Result<()> {
        if self.lines.is_empty() {
            self.read_config_file()?;
        }

        Ok(())
    }

    pub fn open_for_update(&mut self) -> Result<()> {
        self.lockfile.hold_for_update()?;
        self.read_config_file()?;

        Ok(())
    }

    pub fn save(&mut self) -> Result<()> {
        for (_section, lines) in &self.lines {
            for line in lines {
                self.lockfile.write(line.text.as_bytes())?;
            }
        }
        self.lockfile.commit()?;

        Ok(())
    }

    pub fn get(&self, key: &[String]) -> Option<VariableValue> {
        self.get_all(key).last().map(|val| val.to_owned())
    }

    pub fn get_all(&self, key: &[String]) -> Vec<VariableValue> {
        let (key, var) = self.split_key(key);

        let (_, lines) = self.find_lines(&key, &var);

        lines
            .iter()
            .map(|line| line.variable.as_ref().unwrap().value.to_owned())
            .collect()
    }

    pub fn add(&mut self, key: &[String], value: VariableValue) {
        let (key, var) = self.split_key(key);
        let (section, _) = self.find_lines(&key, &var);

        self.add_variable(section, key, var, value);
    }

    pub fn set(&mut self, key: &[String], value: VariableValue) -> Result<()> {
        let (key, var) = self.split_key(key);
        let (section, mut lines) = self.find_lines(&key, &var);

        match lines.len() {
            0 => self.add_variable(section, key, var, value),
            1 => {
                self.update_variable(&mut lines[0], var, value);
            }
            _ => {
                return Err(Error::ConfigConflict(String::from(
                    "cannot overwrite multiple values with a single value",
                )))
            }
        }

        Ok(())
    }

    pub fn replace_all(&mut self, key: &[String], value: VariableValue) {
        let (key, var) = self.split_key(key);
        let (section, lines) = self.find_lines(&key, &var);
        let section = section.unwrap();

        self.remove_all(&section, &lines);
        self.add_variable(Some(section), key, var, value);
    }

    pub fn unset(&mut self, key: &[String]) -> Result<()> {
        self.unset_all(key, |lines| {
            if lines.len() > 1 {
                Err(Error::ConfigConflict(String::from(
                    "key has multiple values",
                )))
            } else {
                Ok(())
            }
        })?;

        Ok(())
    }

    pub fn unset_all<F>(&mut self, key: &[String], f: F) -> Result<()>
    where
        F: Fn(&[Line]) -> Result<()>,
    {
        let (key, var) = self.split_key(key);
        let (section, lines) = self.find_lines(&key, &var);

        if let Some(section) = section {
            f(&lines)?;

            self.remove_all(&section, &lines);
            let lines = self.lines_for(&section);
            if lines.len() == 1 {
                self.remove_section(&key);
            }

            Ok(())
        } else {
            Ok(())
        }
    }

    pub fn remove_section(&mut self, key: &[String]) -> bool {
        let key = Section::normalize(key);

        matches!(self.lines.remove(&key), Some(_))
    }

    pub fn subsections(&self, name: &str) -> Vec<String> {
        let name = &Section::normalize(&[name.to_owned()])[0];
        let mut sections = Vec::new();

        for key in self.lines.keys() {
            assert_eq!(key.len(), 2);
            let (main, sub) = (&key[0], &key[1]);

            if main == name && !sub.is_empty() {
                sections.push(sub.to_owned());
            }
        }

        sections
    }

    pub fn has_section(&self, key: &[String]) -> bool {
        let key = Section::normalize(key);
        self.lines.contains_key(&key)
    }

    fn line_count(&self) -> usize {
        self.lines.values().map(|lines| lines.len()).sum::<usize>() + self.lines.len()
    }

    fn lines_for(&mut self, section: &Section) -> &mut Vec<Line> {
        self.lines
            .entry(Section::normalize(&section.name))
            .or_insert_with(Vec::new)
    }

    fn split_key(&self, key: &[String]) -> (Vec<String>, String) {
        let len = key.len();
        let var = &key[len - 1];

        (key[0..len - 1].to_owned(), var.to_owned())
    }

    fn find_lines(&self, key: &[String], var: &str) -> (Option<Section>, Vec<Line>) {
        let name = Section::normalize(key);

        if let Some(lines) = self.lines.get(&name) {
            let section = &lines[0].section;
            let normal = Variable::normalize(var);

            let lines: Vec<_> = lines
                .iter()
                .filter_map(|l| {
                    if l.normal_variable().as_deref() == Some(&normal) {
                        Some(l.to_owned())
                    } else {
                        None
                    }
                })
                .collect();

            (Some(section.to_owned()), lines)
        } else {
            (None, vec![])
        }
    }

    fn add_section(&mut self, key: &[String]) -> Section {
        let section = Section::new(key.to_owned());
        let line = Line::new(section.heading_line(), section.clone(), None);

        self.lines_for(&section).push(line);
        section
    }

    fn add_variable(
        &mut self,
        section: Option<Section>,
        key: Vec<String>,
        var: String,
        value: VariableValue,
    ) {
        let section = if let Some(section) = section {
            section
        } else {
            self.add_section(&key)
        };

        let text = Variable::serialize(&var, &value);
        let var = Variable::new(var, value);
        let line = Line::new(text, section.clone(), Some(var));

        self.lines_for(&section).push(line);
    }

    fn update_variable(&mut self, line: &mut Line, var: String, value: VariableValue) {
        // Find the position of the line in `self.lines` so we can update that too
        let lines = self.lines_for(&line.section);
        let index = lines.iter().position(|l| l == line).unwrap();

        line.variable.as_mut().unwrap().value = value.clone();
        line.text = Variable::serialize(&var, &value);

        // Update `self.lines` with the updated variable line
        lines[index] = line.clone();
    }

    fn remove_all(&mut self, section: &Section, lines: &[Line]) {
        for line in lines {
            self.lines_for(section).retain(|l| l != line);
        }
    }

    fn read_config_file(&mut self) -> Result<()> {
        let mut section = Section::new(vec![]);

        let file = match File::open(&self.path) {
            Ok(file) => io::BufReader::new(file),
            Err(err) => {
                if err.kind() == io::ErrorKind::NotFound {
                    return Ok(());
                } else {
                    return Err(Error::Io(err));
                }
            }
        };

        // TODO: Support multi-line strings in config values
        for line in file.lines() {
            let mut line = self.parse_line(&section, &line?)?;
            // `file.lines()` strips the newline characters
            line.text.push('\n');
            section = line.section.clone();

            self.lines_for(&section).push(line);
        }

        Ok(())
    }

    fn parse_line(&self, section: &Section, line: &str) -> Result<Line> {
        if let Some(r#match) = SECTION_LINE.captures(line) {
            let mut name = vec![r#match[1].to_owned()];
            if let Some(r#match) = r#match.get(3) {
                name.push(r#match.as_str().to_owned());
            }
            let section = Section::new(name);

            Ok(Line::new(line.to_owned(), section, None))
        } else if let Some(r#match) = VARIABLE_LINE.captures(line) {
            let variable = Variable::new(r#match[1].to_owned(), self.parse_value(&r#match[2]));

            Ok(Line::new(
                line.to_owned(),
                section.to_owned(),
                Some(variable),
            ))
        } else if let Some(_match) = BLANK_LINE.captures(line) {
            Ok(Line::new(line.to_owned(), section.to_owned(), None))
        } else {
            Err(Error::ConfigParseError(
                self.line_count() + 1,
                self.path.clone(),
            ))
        }
    }

    fn parse_value(&self, value: &str) -> VariableValue {
        match value {
            "yes" | "on" | "true" => VariableValue::Bool(true),
            "no" | "off" | "false" => VariableValue::Bool(false),
            _ if INTEGER.is_match(value) => VariableValue::Int(value.parse().unwrap()),
            _ => VariableValue::String(value.replace("\\\n", "")),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use matches::assert_matches;
    use rstest::{fixture, rstest};
    use tempfile::NamedTempFile;

    use super::*;

    #[fixture]
    fn config() -> Config {
        let path = NamedTempFile::new().unwrap().into_temp_path();

        let mut config = Config::new(&path);
        config.open().unwrap();

        config
    }

    #[rstest]
    #[case("yes", VariableValue::Bool(true))]
    #[case("on", VariableValue::Bool(true))]
    #[case("true", VariableValue::Bool(true))]
    #[case("no", VariableValue::Bool(false))]
    #[case("off", VariableValue::Bool(false))]
    #[case("false", VariableValue::Bool(false))]
    #[case("-2", VariableValue::Int(-2))]
    #[case("19", VariableValue::Int(19))]
    #[case("2.3", VariableValue::String(String::from("2.3")))]
    #[case("hello world", VariableValue::String(String::from("hello world")))]
    fn parse_value(config: Config, #[case] input: &str, #[case] expected: VariableValue) {
        assert_eq!(config.parse_value(input), expected);
    }

    mod in_memory {
        use super::*;

        #[rstest]
        fn return_none_for_an_unknown_key(config: Config) {
            assert!(config
                .get(&[String::from("core"), String::from("editor")])
                .is_none());
        }

        #[rstest]
        fn return_the_value_for_a_known_key(mut config: Config) -> Result<()> {
            let key = &[String::from("core"), String::from("editor")];
            let val = VariableValue::String(String::from("ed"));

            config.set(key, val.clone())?;
            assert_eq!(config.get(key), Some(val));

            Ok(())
        }

        #[rstest]
        fn treat_section_names_as_case_insensitive(mut config: Config) -> Result<()> {
            let val = VariableValue::String(String::from("ed"));

            config.set(&[String::from("core"), String::from("editor")], val.clone())?;
            assert_eq!(
                config.get(&[String::from("Core"), String::from("editor")]),
                Some(val)
            );

            Ok(())
        }

        #[rstest]
        fn treat_variable_names_as_case_insensitive(mut config: Config) -> Result<()> {
            let val = VariableValue::String(String::from("ed"));

            config.set(&[String::from("core"), String::from("editor")], val.clone())?;
            assert_eq!(
                config.get(&[String::from("core"), String::from("Editor")]),
                Some(val)
            );

            Ok(())
        }

        #[rstest]
        fn retrieve_values_from_subsections(mut config: Config) -> Result<()> {
            let key = &[
                String::from("branch"),
                String::from("master"),
                String::from("remote"),
            ];
            let val = VariableValue::String(String::from("origin"));

            config.set(key, val.clone())?;
            assert_eq!(config.get(key), Some(val));

            Ok(())
        }

        #[rstest]
        fn treat_subsection_names_as_case_sensitive(mut config: Config) -> Result<()> {
            config.set(
                &[
                    String::from("branch"),
                    String::from("master"),
                    String::from("remote"),
                ],
                VariableValue::String(String::from("origin")),
            )?;
            assert!(config
                .get(&[
                    String::from("branch"),
                    String::from("Master"),
                    String::from("remote"),
                ])
                .is_none());

            Ok(())
        }

        #[rstest]
        fn subsections(mut config: Config) -> Result<()> {
            config.set(
                &[
                    String::from("remote"),
                    String::from("origin"),
                    String::from("url"),
                ],
                VariableValue::String(String::from("ssh://example.com/repo")),
            )?;
            config.set(
                &[
                    String::from("remote"),
                    String::from("github"),
                    String::from("url"),
                ],
                VariableValue::String(String::from("ssh://git@github.com/user/repo")),
            )?;

            assert_eq!(
                config.subsections("remote"),
                vec![String::from("origin"), String::from("github")]
            );

            Ok(())
        }

        mod with_multi_valued_keys {
            use super::*;

            #[fixture]
            fn config() -> Config {
                let path = NamedTempFile::new().unwrap().into_temp_path();

                let mut config = Config::new(&path);
                config.open().unwrap();

                let key = &[
                    String::from("remote"),
                    String::from("origin"),
                    String::from("fetch"),
                ];

                config.add(key, VariableValue::String(String::from("master")));
                config.add(key, VariableValue::String(String::from("topic")));

                config
            }

            #[rstest]
            fn add_multiple_values_for_a_key(config: Config) {
                let key = &[
                    String::from("remote"),
                    String::from("origin"),
                    String::from("fetch"),
                ];

                assert_eq!(
                    config.get(key),
                    Some(VariableValue::String(String::from("topic")))
                );
                assert_eq!(
                    config.get_all(key),
                    vec![
                        VariableValue::String(String::from("master")),
                        VariableValue::String(String::from("topic")),
                    ]
                );
            }

            #[rstest]
            fn refuse_to_set_a_value(mut config: Config) {
                let key = &[
                    String::from("remote"),
                    String::from("origin"),
                    String::from("fetch"),
                ];

                assert_matches!(
                    config.set(key, VariableValue::String(String::from("new-value"))),
                    Err(Error::ConfigConflict(_))
                );
            }

            #[rstest]
            fn replace_all_the_values(mut config: Config) {
                let key = &[
                    String::from("remote"),
                    String::from("origin"),
                    String::from("fetch"),
                ];
                let val = VariableValue::String(String::from("new-value"));

                config.replace_all(key, val.clone());

                assert_eq!(config.get_all(key), vec![val]);
            }

            #[rstest]
            fn refuse_to_unset_a_value(mut config: Config) {
                let key = &[
                    String::from("remote"),
                    String::from("origin"),
                    String::from("fetch"),
                ];

                assert_matches!(config.unset(key), Err(Error::ConfigConflict(_)));
            }

            #[rstest]
            fn unset_all_the_values(mut config: Config) -> Result<()> {
                let key = &[
                    String::from("remote"),
                    String::from("origin"),
                    String::from("fetch"),
                ];

                config.unset_all(key, |_lines| Ok(()))?;
                assert_eq!(config.get_all(key), vec![]);

                Ok(())
            }
        }
    }

    mod file_storage {
        use super::*;

        fn assert_file(config: &Config, contents: &str) -> Result<()> {
            assert_eq!(fs::read_to_string(&config.path)?, contents);

            Ok(())
        }

        #[fixture]
        fn config() -> Config {
            let path = NamedTempFile::new().unwrap().into_temp_path();

            let mut config = Config::new(&path);
            config.open_for_update().unwrap();

            config
        }

        #[rstest]
        fn write_a_single_setting(mut config: Config) -> Result<()> {
            config.set(
                &[String::from("core"), String::from("editor")],
                VariableValue::String(String::from("ed")),
            )?;
            config.save()?;

            assert_file(
                &config,
                "\
[core]
\teditor = ed
",
            )?;

            Ok(())
        }

        #[rstest]
        fn write_multiple_settings(mut config: Config) -> Result<()> {
            config.set(
                &[String::from("core"), String::from("editor")],
                VariableValue::String(String::from("ed")),
            )?;
            config.set(
                &[String::from("user"), String::from("name")],
                VariableValue::String(String::from("A. U. Thor")),
            )?;
            config.set(
                &[String::from("Core"), String::from("bare")],
                VariableValue::Bool(true),
            )?;
            config.save()?;

            assert_file(
                &config,
                "\
[core]
\teditor = ed
\tbare = true
[user]
\tname = A. U. Thor
",
            )?;

            Ok(())
        }

        #[rstest]
        fn write_multiple_subsections(mut config: Config) -> Result<()> {
            config.set(
                &[
                    String::from("branch"),
                    String::from("master"),
                    String::from("remote"),
                ],
                VariableValue::String(String::from("origin")),
            )?;
            config.set(
                &[
                    String::from("branch"),
                    String::from("Master"),
                    String::from("remote"),
                ],
                VariableValue::String(String::from("another")),
            )?;
            config.save()?;

            assert_file(
                &config,
                "\
[branch \"master\"]
\tremote = origin
[branch \"Master\"]
\tremote = another
",
            )?;

            Ok(())
        }

        #[rstest]
        fn overwrite_a_variable_with_a_matching_name(mut config: Config) -> Result<()> {
            config.set(
                &[String::from("merge"), String::from("conflictstyle")],
                VariableValue::String(String::from("diff3")),
            )?;
            config.set(
                &[String::from("merge"), String::from("ConflictStyle")],
                VariableValue::String(String::from("none")),
            )?;
            config.save()?;

            assert_file(
                &config,
                "\
[merge]
\tConflictStyle = none
",
            )?;

            Ok(())
        }

        #[rstest]
        fn remove_a_section(mut config: Config) -> Result<()> {
            config.set(
                &[String::from("core"), String::from("editor")],
                VariableValue::String(String::from("ed")),
            )?;
            config.set(
                &[
                    String::from("remote"),
                    String::from("origin"),
                    String::from("url"),
                ],
                VariableValue::String(String::from("ssh://example.com/repo")),
            )?;
            config.remove_section(&[String::from("core")]);
            config.save()?;

            assert_file(
                &config,
                "\
[remote \"origin\"]
\turl = ssh://example.com/repo
",
            )?;

            Ok(())
        }

        #[rstest]
        fn remove_a_subsection(mut config: Config) -> Result<()> {
            config.set(
                &[String::from("core"), String::from("editor")],
                VariableValue::String(String::from("ed")),
            )?;
            config.set(
                &[
                    String::from("remote"),
                    String::from("origin"),
                    String::from("url"),
                ],
                VariableValue::String(String::from("ssh://example.com/repo")),
            )?;
            config.remove_section(&[String::from("remote"), String::from("origin")]);
            config.save()?;

            assert_file(
                &config,
                "\
[core]
\teditor = ed
",
            )?;

            Ok(())
        }

        #[rstest]
        fn unset_a_variable(mut config: Config) -> Result<()> {
            config.set(
                &[String::from("merge"), String::from("conflictstyle")],
                VariableValue::String(String::from("diff3")),
            )?;
            config.unset(&[String::from("merge"), String::from("ConflictStyle")])?;
            config.save()?;

            assert_file(&config, "")?;

            Ok(())
        }

        #[rstest]
        fn retrieve_persisted_settings(mut config: Config) -> Result<()> {
            let key = &[String::from("core"), String::from("editor")];
            let val = VariableValue::String(String::from("ed"));

            config.set(key, val.clone())?;
            config.save()?;

            let mut open_config = Config::new(&config.path);
            open_config.open()?;

            assert_eq!(open_config.get(key), Some(val));

            Ok(())
        }

        #[rstest]
        fn retrieve_variables_from_subsections(mut config: Config) -> Result<()> {
            config.set(
                &[
                    String::from("branch"),
                    String::from("master"),
                    String::from("remote"),
                ],
                VariableValue::String(String::from("origin")),
            )?;
            config.set(
                &[
                    String::from("branch"),
                    String::from("Master"),
                    String::from("remote"),
                ],
                VariableValue::String(String::from("another")),
            )?;
            config.save()?;

            let mut open_config = Config::new(&config.path);
            open_config.open()?;

            assert_eq!(
                open_config.get(&[
                    String::from("branch"),
                    String::from("master"),
                    String::from("remote")
                ]),
                Some(VariableValue::String(String::from("origin")))
            );
            assert_eq!(
                open_config.get(&[
                    String::from("branch"),
                    String::from("Master"),
                    String::from("remote")
                ]),
                Some(VariableValue::String(String::from("another")))
            );

            Ok(())
        }

        #[rstest]
        fn retreive_variables_from_subsections_including_dots(mut config: Config) -> Result<()> {
            let key = &[
                String::from("url"),
                String::from("git@github.com:"),
                String::from("insteadOf"),
            ];
            let val = VariableValue::String(String::from("gh:"));

            config.set(key, val.clone())?;
            config.save()?;

            let mut open_config = Config::new(&config.path);
            open_config.open()?;

            assert_eq!(open_config.get(key), Some(val));

            Ok(())
        }

        #[rstest]
        fn retain_the_formatting_of_existing_settings(mut config: Config) -> Result<()> {
            config.set(
                &[String::from("core"), String::from("Editor")],
                VariableValue::String(String::from("ed")),
            )?;
            config.set(
                &[String::from("user"), String::from("Name")],
                VariableValue::String(String::from("A. U. Thor")),
            )?;
            config.set(
                &[String::from("Core"), String::from("Bare")],
                VariableValue::Bool(true),
            )?;
            config.save()?;

            let mut config = Config::new(&config.path);
            config.open_for_update()?;
            config.set(
                &[String::from("Core"), String::from("bare")],
                VariableValue::Bool(false),
            )?;
            config.save()?;

            assert_file(
                &config,
                "\
[core]
\tEditor = ed
\tbare = false
[user]
\tName = A. U. Thor
",
            )?;

            Ok(())
        }
    }
}
