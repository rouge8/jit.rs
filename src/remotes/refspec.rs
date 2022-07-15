use std::fmt;
use std::path::PathBuf;

use crate::util::path_to_string;

pub struct Refspec {
    source: PathBuf,
    target: PathBuf,
    forced: bool,
}

impl Refspec {
    pub fn new(source: PathBuf, target: PathBuf, forced: bool) -> Self {
        Self {
            source,
            target,
            forced,
        }
    }
}

impl fmt::Display for Refspec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let spec = if self.forced { "+" } else { "" };
        write!(
            f,
            "{}{}:{}",
            spec,
            path_to_string(&self.source),
            path_to_string(&self.target),
        )
    }
}
