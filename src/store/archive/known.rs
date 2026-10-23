//! bird's keys as a note's region reads them.
//!
//! A key the observation lacks is read from the stored note, the value the
//! merged frontmatter keeps, so a response that omits a field or an
//! `includes` entry never takes from the region what the note already shows.

use serde_json::{Map, Value};

use super::frontmatter::{Stored, region_parts};
use super::yaml::Field;

/// An observation's keys over the stored note's frontmatter and region.
pub struct Known<'a> {
    observed: &'a [(&'static str, Field)],
    stored: Option<&'a Map<String, Value>>,
    region: Option<&'a str>,
}

impl<'a> Known<'a> {
    /// `observed` over `stored`, when the note exists.
    pub fn new(observed: &'a [(&'static str, Field)], stored: Option<&'a Stored<'a>>) -> Self {
        Self {
            observed,
            stored: stored.map(|s| &s.parsed),
            region: stored.and_then(|s| region_parts(s.body).1),
        }
    }

    fn observed(&self, key: &str) -> Option<&'a Field> {
        self.observed
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v)
    }

    fn stored(&self, key: &str) -> Option<&'a Value> {
        self.stored.and_then(|s| s.get(key))
    }

    /// The string `key` holds.
    pub fn str(&self, key: &str) -> Option<&'a str> {
        match self.observed(key) {
            Some(Field::Str(s)) => Some(s),
            Some(_) => None,
            None => self.stored(key).and_then(Value::as_str),
        }
    }

    /// The strings list `key` holds.
    pub fn list(&self, key: &str) -> Vec<&'a str> {
        match self.observed(key) {
            Some(Field::List(items)) => items.iter().map(String::as_str).collect(),
            Some(_) => Vec::new(),
            None => self
                .stored(key)
                .and_then(Value::as_array)
                .map(|items| items.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default(),
        }
    }

    /// The first line of the stored region that `matches` accepts.
    pub fn stored_line(&self, matches: impl Fn(&str) -> bool) -> Option<&'a str> {
        self.region?.lines().find(|line| matches(line))
    }
}
