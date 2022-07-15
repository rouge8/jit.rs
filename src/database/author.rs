use std::fmt;

use chrono::{DateTime, FixedOffset};
use itertools::Itertools;

const TIME_FORMAT: &str = "%s %z";

#[derive(Debug, Clone)]
pub struct Author {
    pub name: String,
    pub email: String,
    pub time: DateTime<FixedOffset>,
}

impl Author {
    pub fn new(name: String, email: String, time: DateTime<FixedOffset>) -> Self {
        Author { name, email, time }
    }

    pub fn parse(data: &str) -> Self {
        let (name, email, time) = data.splitn(3, &['<', '>'][..]).collect_tuple().unwrap();

        let time = time.trim();

        Author {
            name: name.trim().to_string(),
            email: email.to_string(),
            time: DateTime::parse_from_str(time, TIME_FORMAT)
                .expect("Could not parse author timestamp"),
        }
    }

    pub fn short_date(&self) -> String {
        self.time.format("%Y-%m-%d").to_string()
    }

    pub fn readable_time(&self) -> String {
        self.time.format("%a %b %-d %H:%M:%S %Y %z").to_string()
    }
}

impl fmt::Display for Author {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let timestamp = self.time.format(TIME_FORMAT);
        write!(f, "{} <{}> {}", self.name, self.email, timestamp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_display_roundtrip() {
        let display = "A. U. Thor <author@example.com> 1624680163 -0700";

        let author = Author::parse(display);
        assert_eq!(author.name, "A. U. Thor");
        assert_eq!(author.email, "author@example.com");

        assert_eq!(author.to_string(), display);
    }
}
