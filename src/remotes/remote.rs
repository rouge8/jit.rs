use crate::config::{Config, VariableValue};
use crate::errors::Result;
use std::cell::RefCell;
use std::rc::Rc;

pub struct Remote {
    config: Rc<RefCell<Config>>,
    name: String,
}

impl Remote {
    pub fn new(config: Rc<RefCell<Config>>, name: &str) -> Result<Self> {
        config.borrow_mut().open()?;

        Ok(Self {
            config,
            name: name.to_owned(),
        })
    }

    pub fn fetch_url(&self) -> Option<VariableValue> {
        self.config.borrow().get(&[
            String::from("remote"),
            self.name.to_string(),
            String::from("url"),
        ])
    }

    pub fn fetch_specs(&self) -> Option<VariableValue> {
        self.config.borrow().get(&[
            String::from("remote"),
            self.name.to_string(),
            String::from("fetch"),
        ])
    }

    pub fn push_url(&self) -> Option<VariableValue> {
        if let Some(push_url) = self.config.borrow().get(&[
            String::from("remote"),
            self.name.to_string(),
            String::from("pushurl"),
        ]) {
            Some(push_url)
        } else {
            self.fetch_url()
        }
    }

    pub fn uploader(&self) -> Option<VariableValue> {
        self.config.borrow().get(&[
            String::from("remote"),
            self.name.to_string(),
            String::from("uploadpack"),
        ])
    }
}
