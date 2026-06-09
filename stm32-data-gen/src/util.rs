use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::Hash;
use std::ptr;
use std::sync::{Mutex, OnceLock};

use regex::Regex;

pub trait HashMapFns<K: Eq + Hash, V> {
    fn try_insert_stable(&mut self, key: K, value: V) -> Result<(), ()>;
    fn get_or_try_insert_with(&mut self, key: K, f: impl FnOnce() -> anyhow::Result<V>) -> anyhow::Result<&V>;
}

impl<K: Eq + Hash, V> HashMapFns<K, V> for HashMap<K, V> {
    fn try_insert_stable(&mut self, key: K, value: V) -> Result<(), ()> {
        match self.entry(key) {
            Entry::Occupied(_) => Err(()),
            Entry::Vacant(entry) => {
                entry.insert(value);

                Ok(())
            }
        }
    }

    fn get_or_try_insert_with(&mut self, key: K, f: impl FnOnce() -> anyhow::Result<V>) -> anyhow::Result<&V> {
        match self.entry(key) {
            Entry::Occupied(e) => Ok(e.into_mut()),
            Entry::Vacant(e) => Ok(e.insert(f()?)),
        }
    }
}

pub trait EntryFns<'a, K: Eq + Hash + 'a, V: 'a> {
    fn or_insert_with_mut(&mut self, f: impl FnOnce() -> V) -> &mut V;
}

impl<'a, K: Eq + Hash + 'a, V: 'a> EntryFns<'a, K, V> for Entry<'a, K, V> {
    fn or_insert_with_mut(&mut self, f: impl FnOnce() -> V) -> &mut V {
        unsafe {
            let e = if let Entry::Vacant(e) = self {
                let r = f(); // If the function panics, there will not be a duplicate entry during unwind

                Some(Entry::Occupied(ptr::read(e).insert_entry(r)))
            } else {
                None
            };

            e.map(|e| ptr::write(self, e));
        }

        match self {
            Entry::Occupied(e) => e.get_mut(),
            _ => unreachable!(),
        }
    }
}

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

    pub const fn get_map(&self) -> &'a [(&'a str, T)] {
        self.map
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

pub struct RegexSet<'a> {
    map: RegexMap<'a, ()>,
}

impl<'a> RegexSet<'a> {
    pub const fn new(map: &'a [&'a str]) -> Self {
        Self {
            map: RegexMap::new(unsafe { std::mem::transmute::<&[&str], &[(&str, ())]>(map) }),
        }
    }

    pub fn contains(&self, key: &str) -> bool {
        self.map.get(key).is_some()
    }
}
