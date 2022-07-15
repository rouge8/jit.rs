use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::config::{Config, VariableValue};
use crate::errors::Result;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum ConfigFile {
    Local,
    Global,
    System,
    File(PathBuf),
}

#[derive(Debug)]
pub struct Stack {
    configs: HashMap<ConfigFile, Rc<RefCell<Config>>>,
}

impl Stack {
    pub fn new(git_path: &Path) -> Self {
        let configs = HashMap::from([
            (
                ConfigFile::Local,
                Rc::new(RefCell::new(Config::new(&git_path.join("config")))),
            ),
            (
                ConfigFile::Global,
                Rc::new(RefCell::new(Config::new(
                    &dirs::home_dir()
                        .unwrap_or_else(|| PathBuf::from("/"))
                        .join(".gitconfig"),
                ))),
            ),
            (
                ConfigFile::System,
                Rc::new(RefCell::new(Config::new(&PathBuf::from("/etc/gitconfig")))),
            ),
        ]);

        Self { configs }
    }

    pub fn file(&mut self, name: ConfigFile) -> Rc<RefCell<Config>> {
        match name {
            ConfigFile::Local => Rc::clone(&self.configs[&ConfigFile::Local]),
            ConfigFile::Global => Rc::clone(&self.configs[&ConfigFile::Global]),
            ConfigFile::System => Rc::clone(&self.configs[&ConfigFile::System]),
            ConfigFile::File(path) => {
                self.configs.insert(
                    ConfigFile::File(path.clone()),
                    Rc::new(RefCell::new(Config::new(&path))),
                );
                Rc::clone(&self.configs[&ConfigFile::File(path)])
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
