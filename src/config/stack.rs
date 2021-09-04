use crate::config::{Config, VariableValue};
use crate::errors::Result;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum ConfigFile {
    Local,
    Global,
    System,
    File(PathBuf),
}

#[derive(Debug)]
pub struct Stack {
    configs: HashMap<ConfigFile, RefCell<Config>>,
}

impl Stack {
    pub fn new(git_path: &Path) -> Self {
        let mut configs = HashMap::new();
        configs.insert(
            ConfigFile::Local,
            RefCell::new(Config::new(&git_path.join("config"))),
        );
        configs.insert(
            ConfigFile::Global,
            RefCell::new(Config::new(
                &dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("/"))
                    .join(".gitconfig"),
            )),
        );
        configs.insert(
            ConfigFile::System,
            RefCell::new(Config::new(&PathBuf::from("/etc/gitconfig"))),
        );

        Self { configs }
    }

    pub fn file(&mut self, name: ConfigFile) -> &RefCell<Config> {
        match name {
            ConfigFile::Local => &self.configs[&ConfigFile::Local],
            ConfigFile::Global => &self.configs[&ConfigFile::Global],
            ConfigFile::System => &self.configs[&ConfigFile::System],
            ConfigFile::File(path) => {
                self.configs.insert(
                    ConfigFile::File(path.clone()),
                    RefCell::new(Config::new(&path)),
                );
                &self.configs[&ConfigFile::File(path)]
            }
        }
    }

    pub fn open(&self) -> Result<()> {
        for config in self.configs.values() {
            let mut config = config.borrow_mut();
            config.open()?;
        }

        Ok(())
    }

    pub fn get(&self, key: &[String]) -> Option<VariableValue> {
        self.get_all(key).last().map(|val| val.to_owned())
    }

    pub fn get_all(&self, key: &[String]) -> Vec<VariableValue> {
        [ConfigFile::System, ConfigFile::Global, ConfigFile::Local]
            .iter()
            .flat_map(|name| {
                let mut config = self.configs[name].borrow_mut();
                config.open().unwrap();
                config.get_all(key)
            })
            .collect()
    }
}
