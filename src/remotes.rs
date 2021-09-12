use crate::config::{Config, VariableValue};
use crate::errors::{Error, Result};
use crate::refs::{HEADS_DIR, REMOTES_DIR};
use refspec::Refspec;
use remote::Remote;
use std::cell::RefCell;
use std::rc::Rc;

mod refspec;
mod remote;

static DEFAULT_REMOTE: &str = "origin";

#[derive(Debug)]
pub struct Remotes {
    config: Rc<RefCell<Config>>,
}

impl Remotes {
    pub fn new(config: Rc<RefCell<Config>>) -> Self {
        Self { config }
    }

    pub fn add(&self, name: &str, url: &str, branches: &[String]) -> Result<()> {
        let branches = if branches.is_empty() {
            vec![String::from("*")]
        } else {
            branches.to_owned()
        };

        let mut config = self.config.borrow_mut();
        config.open_for_update()?;

        if config
            .get(&[
                String::from("remote"),
                name.to_string(),
                String::from("url"),
            ])
            .is_some()
        {
            return Err(Error::InvalidRemote(format!(
                "remote {} already exists.",
                name
            )));
        }

        config.set(
            &[
                String::from("remote"),
                name.to_string(),
                String::from("url"),
            ],
            VariableValue::String(url.to_string()),
        )?;

        for branch in branches {
            let source = HEADS_DIR.join(&branch);
            let target = REMOTES_DIR.join(name).join(&branch);
            let refspec = Refspec::new(source, target, true);

            config.add(
                &[
                    String::from("remote"),
                    name.to_string(),
                    String::from("fetch"),
                ],
                VariableValue::String(refspec.to_string()),
            );
        }

        config.save()?;

        Ok(())
    }

    pub fn remove(&self, name: &str) -> Result<()> {
        let mut config = self.config.borrow_mut();
        config.open_for_update()?;

        if !config.remove_section(&[String::from("remote"), name.to_string()]) {
            config.save()?;
            return Err(Error::InvalidRemote(format!("No such remote: {}", name)));
        }

        config.save()?;

        Ok(())
    }

    pub fn list_remotes(&self) -> Result<Vec<String>> {
        let mut config = self.config.borrow_mut();
        config.open()?;

        Ok(config.subsections("remote"))
    }

    pub fn get(&self, name: &str) -> Result<Option<Remote>> {
        {
            let mut config = self.config.borrow_mut();
            config.open()?;
            if !config.has_section(&[String::from("remote"), name.to_string()]) {
                return Ok(None);
            }
        }

        Ok(Some(Remote::new(Rc::clone(&self.config), name)?))
    }
}
