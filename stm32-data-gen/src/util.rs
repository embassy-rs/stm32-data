use regex::Regex;

pub struct RegexMap<T: 'static> {
    map: &'static [(&'static str, T)],
}

impl<T: 'static> RegexMap<T> {
    pub const fn new(map: &'static [(&'static str, T)]) -> Self {
        Self { map }
    }

    pub fn get(&self, key: &str) -> Option<&T> {
        for (k, v) in self.map {
            if Regex::new(&format!("^{k}$")).unwrap().is_match(key) {
                return Some(v);
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
