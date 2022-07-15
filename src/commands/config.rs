use std::cell::RefMut;

use crate::commands::{Command, CommandContext};
use crate::config::stack::{ConfigFile, Stack};
use crate::config::{Config, VariableValue};
use crate::errors::{Error, Result};

pub struct ConfigCommand<'a> {
    ctx: CommandContext<'a>,
    file: Option<ConfigFile>,
    mode: Option<Mode>,
    raw_key: String,
    value: Option<String>,
}

#[derive(Debug)]
enum Mode {
    Add,
    Replace,
    GetAll,
    Unset,
    UnsetAll,
    RemoveSection,
}

#[derive(Debug)]
enum ConfigOrStack<'a> {
    Config(&'a RefMut<'a, Config>),
    Stack(&'a Stack),
}

impl<'a> ConfigCommand<'a> {
    pub fn new(ctx: CommandContext<'a>) -> Self {
        let (file, mode, raw_key, value) = match &ctx.opt.cmd {
            Command::Config {
                args,
                local,
                global,
                system,
                file,
                add,
                replace_all,
                get_all,
                unset,
                unset_all,
                remove_section,
            } => {
                let config_file = if *local {
                    Some(ConfigFile::Local)
                } else if *global {
                    Some(ConfigFile::Global)
                } else if *system {
                    Some(ConfigFile::System)
                } else {
                    file.as_ref().map(|file| ConfigFile::File(file.to_owned()))
                };

                let (mode, raw_key, value) = if let Some(raw_key) = add {
                    (
                        Some(Mode::Add),
                        raw_key.to_owned(),
                        Some(args[0].to_owned()),
                    )
                } else if let Some(raw_key) = replace_all {
                    (
                        Some(Mode::Replace),
                        raw_key.to_owned(),
                        Some(args[0].to_owned()),
                    )
                } else if let Some(raw_key) = get_all {
                    (Some(Mode::GetAll), raw_key.to_owned(), None)
                } else if let Some(raw_key) = unset {
                    (Some(Mode::Unset), raw_key.to_owned(), None)
                } else if let Some(raw_key) = unset_all {
                    (Some(Mode::UnsetAll), raw_key.to_owned(), None)
                } else if let Some(raw_key) = remove_section {
                    (Some(Mode::RemoveSection), raw_key.to_owned(), None)
                } else {
                    (
                        None,
                        args[0].to_owned(),
                        args.get(1).map(|arg| arg.to_owned()),
                    )
                };

                (config_file, mode, raw_key, value)
            }
            _ => unreachable!(),
        };

        Self {
            ctx,
            file,
            mode,
            raw_key,
            value,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        let value = self.value.clone();

        match self.mode {
            Some(Mode::Add) => self.add_variable(value.as_deref().unwrap())?,
            Some(Mode::Replace) => self.replace_variable(value.as_deref().unwrap())?,
            Some(Mode::GetAll) => self.get_all_values()?,
            Some(Mode::Unset) => self.unset_single()?,
            Some(Mode::UnsetAll) => self.unset_all()?,
            Some(Mode::RemoveSection) => self.remove_section()?,
            None => {
                let key = self.parse_key(&self.raw_key)?;

                if let Some(value) = value {
                    self.edit_config(|config| {
                        config.set(&key, VariableValue::String(value.clone()))
                    })?;
                } else {
                    self.read_config(|config_or_stack| match config_or_stack {
                        ConfigOrStack::Config(config) => {
                            config.get(&key).map_or_else(Vec::new, |value| vec![value])
                        }
                        ConfigOrStack::Stack(stack) => {
                            stack.get(&key).map_or_else(Vec::new, |value| vec![value])
                        }
                    })?;
                }
            }
        }

        Ok(())
    }

    fn add_variable(&mut self, value: &str) -> Result<()> {
        let key = self.parse_key(&self.raw_key)?;
        self.edit_config(|config| {
            config.add(&key, VariableValue::String(value.to_owned()));

            Ok(())
        })
    }

    fn replace_variable(&mut self, value: &str) -> Result<()> {
        let key = self.parse_key(&self.raw_key)?;
        self.edit_config(|config| {
            config.replace_all(&key, VariableValue::String(value.to_owned()));

            Ok(())
        })
    }

    fn unset_single(&mut self) -> Result<()> {
        let key = self.parse_key(&self.raw_key)?;
        self.edit_config(|config| config.unset(&key))
    }

    fn unset_all(&mut self) -> Result<()> {
        let key = self.parse_key(&self.raw_key)?;
        self.edit_config(|config| config.unset_all(&key, |_lines| Ok(())))
    }

    fn remove_section(&mut self) -> Result<()> {
        let key = self.raw_key.splitn(2, '.');
        let key: Vec<_> = key.map(|k| k.to_owned()).collect();

        self.edit_config(|config| {
            config.remove_section(&key);

            Ok(())
        })
    }

    fn get_all_values(&mut self) -> Result<()> {
        let key = self.parse_key(&self.raw_key)?;
        self.read_config(|config_or_stack| match config_or_stack {
            ConfigOrStack::Config(config) => config.get_all(&key),
            ConfigOrStack::Stack(stack) => stack.get_all(&key),
        })
    }

    fn read_config<F>(&mut self, f: F) -> Result<()>
    where
        F: Fn(ConfigOrStack) -> Vec<VariableValue>,
    {
        let values = if let Some(file) = &self.file {
            let config = self.ctx.repo.config.file(file.clone());
            let mut config = config.borrow_mut();

            config.open()?;
            f(ConfigOrStack::Config(&config))
        } else {
            self.ctx.repo.config.open()?;
            f(ConfigOrStack::Stack(&self.ctx.repo.config))
        };

        if values.is_empty() {
            Err(Error::Exit(1))
        } else {
            let mut stdout = self.ctx.stdout.borrow_mut();

            for value in values {
                writeln!(stdout, "{}", value)?;
            }

            Err(Error::Exit(0))
        }
    }

    fn edit_config<F>(&mut self, f: F) -> Result<()>
    where
        F: Fn(&mut RefMut<Config>) -> Result<()>,
    {
        let file = if let Some(file) = &self.file {
            file.clone()
        } else {
            ConfigFile::Local
        };

        let config = self.ctx.repo.config.file(file);
        let mut config = config.borrow_mut();
        config.open_for_update()?;
        match f(&mut config) {
            Ok(()) => (),
            Err(err) => match err {
                Error::ConfigConflict(..) => {
                    let mut stderr = self.ctx.stderr.borrow_mut();
                    writeln!(stderr, "error: {}", err)?;
                    return Err(Error::Exit(5));
                }
                _ => return Err(err),
            },
        }
        config.save()?;

        Err(Error::Exit(0))
    }

    fn parse_key(&self, name: &str) -> Result<Vec<String>> {
        let split: Vec<_> = name.split('.').collect();

        let section = split[0].to_owned();
        let subsection = if split.len() > 2 {
            split[1..split.len() - 1].to_vec()
        } else {
            Vec::new()
        };

        if split.len() < 2 {
            let mut stderr = self.ctx.stderr.borrow_mut();
            writeln!(stderr, "error: key does not contain a section: {}", name)?;
            return Err(Error::Exit(2));
        }
        let var = split.last().unwrap().to_string();

        if !Config::is_valid_key(&[&section, &var]) {
            let mut stderr = self.ctx.stderr.borrow_mut();
            writeln!(stderr, "error: invalid key: {}", name)?;
            return Err(Error::Exit(1));
        }

        if subsection.is_empty() {
            Ok(vec![section, var])
        } else {
            Ok(vec![section, subsection.join("."), var])
        }
    }
}
