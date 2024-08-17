use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use regex::Regex;

pub struct RegexMap<'a, T> {
    map: &'a [(&'a str, T)],
    regexes: OnceLock<Vec<Regex>>,
    cache: Mutex<Option<HashMap<String, Option<usize>>>>,
}

impl<'a, T> RegexMap<'a, T> {
    pub const fn new(map: &'a [(&'a str, T)]) -> Self {
        Self {
            map,
            regexes: OnceLock::new(),
            cache: Mutex::new(None),
        }
    }

    pub fn get(&self, key: &str) -> Option<&'a T> {
        if let Some(&val) = self.cache.lock().unwrap().get_or_insert_with(Default::default).get(key) {
            return val.map(|i| &self.map[i].1);
        }
        let val = self.get_uncached(key);
        self.cache
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .insert(key.to_string(), val);
        val.map(|i| &self.map[i].1)
    }

    fn get_uncached(&self, key: &str) -> Option<usize> {
        let regexes = self.regexes.get_or_init(|| {
            self.map
                .iter()
                .map(|(k, _)| Regex::new(&format!("^{k}$")).unwrap())
                .collect()
        });

        for (i, k) in regexes.iter().enumerate() {
            if k.is_match(key) {
                return Some(i);
            }
        }
        None
    }

    #[track_caller]
    pub fn must_get(&self, key: &str) -> &T {
        let Some(res) = self.get(key) else {
            panic!("no regexmap for key '{key}'")
        };
        res
    }
}
