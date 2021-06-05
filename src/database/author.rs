use chrono::{DateTime, FixedOffset};
use itertools::Itertools;
use std::fmt;

#[derive(Debug)]
pub struct Author {
    name: String,
    email: String,
    time: DateTime<FixedOffset>,
}

impl Author {
    pub fn new(name: String, email: String, time: DateTime<FixedOffset>) -> Self {
        Author { name, email, time }
    }

    pub fn parse(data: &str) -> Self {
        let (name, email, time) = data.splitn(3, &['<', '>'][..]).collect_tuple().unwrap();

        let time = time.trim();

        Author {
            name: name.to_string(),
            email: email.to_string(),
            time: DateTime::parse_from_str(time, "%s %z")
                .expect("Could not parse author timestamp"),
        }
    }
}

impl fmt::Display for Author {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let timestamp = self.time.format("%s %z");
        write!(f, "{} <{}> {}", self.name, self.email, timestamp)
    }
}
