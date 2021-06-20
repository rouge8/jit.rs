use chrono::{DateTime, FixedOffset};
use itertools::Itertools;
use std::fmt;

const TIME_FORMAT: &str = "%s %z";

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
            time: DateTime::parse_from_str(time, TIME_FORMAT)
                .expect("Could not parse author timestamp"),
        }
    }

    pub fn short_date(&self) -> String {
        self.time.format("%Y-%m-%d").to_string()
    }
}

impl fmt::Display for Author {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let timestamp = self.time.format(TIME_FORMAT);
        write!(f, "{} <{}> {}", self.name, self.email, timestamp)
    }
}
