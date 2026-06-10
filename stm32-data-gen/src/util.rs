use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::Hash;
use std::ptr;

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

pub fn new_regex_map<I, S, V>(items: I) -> regex_map::RegexMap<V>
where
    I: IntoIterator<Item = (S, V)>,
    S: AsRef<str>,
{
    regex_map::RegexMap::new(items.into_iter().map(|(k, v)| (format!("^{}$", k.as_ref()), v)))
}

pub fn new_regex_set<I, S>(items: I) -> regex::RegexSet
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    regex::RegexSet::new(items.into_iter().map(|k| format!("^{}$", k.as_ref()))).unwrap()
}
