use std::collections::HashMap;

use anyhow::{Context, anyhow};
use lazy_regex::regex;
use regex_map::RegexMap;

use crate::config;

pub struct Headers {
    map: HeaderMap,
    parsed: HeadersParsed,
    regexes: RegexMap<String>,
}

impl Headers {
    pub fn parse(filter: &Option<String>) -> anyhow::Result<Self> {
        let map = HeaderMap::parse()?;
        let parsed = HeadersParsed::parse(filter)?;
        let regexes = RegexMap::new(parsed.0.keys().map(|h| {
            let pattern = h.replace('x', ".");

            (format!("^{pattern}$"), h.clone())
        }));

        for v in map.0.values() {
            if let Some(filter) = filter
                && !v.starts_with(filter)
            {
                continue;
            } else if !parsed.0.contains_key(v) {
                return Err(anyhow!(
                    "parsed headers do not contain {v}, specified in the header map",
                ));
            }
        }

        Ok(Self { map, parsed, regexes })
    }

    pub fn get(&self, header: &str) -> Option<&ParsedHeader> {
        self.parsed.0.get(header)
    }

    pub fn get_for_chip(&self, model: &str) -> Option<&ParsedHeader> {
        let model = model.to_ascii_lowercase();
        match self.map.0.get(&model) {
            // if it's in the map, just go
            Some(name) => Some(self.parsed.0.get(name).unwrap()),
            // if not, find it by regex, taking `x` meaning `anything`
            None => {
                let mut results = self.regexes.get(&model);
                let res = results.next();
                assert_eq!(results.next(), None, "found more than one match");
                res.map(|name| self.parsed.0.get(name).unwrap())
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct HeaderMap(pub HashMap<String, String>);

impl HeaderMap {
    pub fn parse() -> anyhow::Result<Self> {
        let mut res = HashMap::new();
        let path = "data/header_map.yaml";
        let header = std::fs::read_to_string(path).with_context(|| format!("Failed to read {}", path))?;
        let header = serde_yaml::from_str::<HashMap<String, String>>(&header)
            .with_context(|| format!("Failed to parse {}", path))?;
        for (mut header, chips) in header {
            header.make_ascii_lowercase();
            for chip in chips.split(',') {
                let chip = chip.trim().to_ascii_lowercase();
                if let Some(old) = res.insert(chip.clone(), header.clone()) {
                    panic!("Duplicate {chip} found in {path}! Overwriting {old} with {header}");
                }
            }
        }

        Ok(Self(res))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct HeadersParsed(pub HashMap<String, ParsedHeader>);

impl HeadersParsed {
    pub fn parse(filter: &Option<String>) -> anyhow::Result<Self> {
        let files = glob::glob("sources/headers/*.h")
            .unwrap()
            .map(Result::unwrap)
            .filter(|f| {
                if let Some(filter) = filter
                    && let Some(filename) = f.file_name()
                    && !filename.to_ascii_lowercase().to_string_lossy().starts_with(filter)
                {
                    false
                } else {
                    true
                }
            });

        let for_each_file = |f: std::path::PathBuf| {
            let ff = f.file_name().unwrap().to_string_lossy();
            let ff = ff.strip_suffix(".h").unwrap();
            let parsed_header = ParsedHeader::parse(&f).unwrap();
            (ff.to_string(), parsed_header)
        };

        #[cfg(feature = "rayon")]
        {
            use rayon::prelude::*;
            Ok(Self(files.par_bridge().map(for_each_file).collect()))
        }
        #[cfg(not(feature = "rayon"))]
        {
            Ok(Self(files.map(for_each_file).collect()))
        }
    }
}

fn parens_ok(val: &str) -> bool {
    let mut n: i32 = 0;
    for c in val.chars() {
        match c {
            '(' => n += 1,
            ')' => {
                n -= 1;
                if n < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    n == 0
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Defines(pub HashMap<String, i64>);

impl Defines {
    // warning: horrible abomination ahead
    fn parse_value(&self, val: &str) -> anyhow::Result<i64> {
        let val = val.trim();
        if val.is_empty() {
            Ok(0)
        } else if let Some(m) = regex!(r"^(0([1-9][0-9]*)(U))").captures(val) {
            m.get(2)
                .unwrap()
                .as_str()
                .parse()
                .map_err(|e| anyhow::anyhow!("Failed to parse {val}: {e}"))
        } else if let Some(m) = regex!(r"^((0x[0-9a-fA-F]+|\d+))(|u|ul|U|UL)$").captures(val) {
            let x = m.get(1).unwrap().as_str();
            match x.strip_prefix("0x") {
                Some(x) => i64::from_str_radix(x, 16).map_err(|e| anyhow::anyhow!("Failed to parse {val}: {e}")),
                None => x.parse().map_err(|e| anyhow::anyhow!("Failed to parse {val}: {e}")),
            }
        } else if let Some(m) = regex!(r"^([0-9A-Za-z_]+)$").captures(val) {
            Ok(self.0.get(m.get(1).unwrap().as_str()).copied().unwrap_or(0))
        } else if let Some(x) = regex!(r"^\((.*)\)$")
            .captures(val)
            .map(|m| m.get(1).unwrap().as_str())
            .filter(|x| parens_ok(x))
        {
            self.parse_value(x)
        } else if let Some(m) = regex!(r"^\*?\([0-9A-Za-z_]+ *\*?\)(.*)$").captures(val) {
            self.parse_value(m.get(1).unwrap().as_str())
        } else if let Some(m) = regex!(r"^(.*)\*(.*)$").captures(val) {
            Ok(self.parse_value(m.get(1).unwrap().as_str())? * self.parse_value(m.get(2).unwrap().as_str())?)
        } else if let Some(m) = regex!(r"^(.*)/(.*)$").captures(val) {
            Ok(self.parse_value(m.get(1).unwrap().as_str())? / self.parse_value(m.get(2).unwrap().as_str())?)
        } else if let Some(m) = regex!(r"^(.*)<<(.*)$").captures(val) {
            Ok(
                self.parse_value(m.get(1).unwrap().as_str())? << self.parse_value(m.get(2).unwrap().as_str())?
                    & 0xFFFFFFFF,
            )
        } else if let Some(m) = regex!(r"^(.*)>>(.*)$").captures(val) {
            Ok(self.parse_value(m.get(1).unwrap().as_str())? >> self.parse_value(m.get(2).unwrap().as_str())?)
        } else if let Some(m) = regex!(r"^(.*)\|(.*)$").captures(val) {
            Ok(self.parse_value(m.get(1).unwrap().as_str())? | self.parse_value(m.get(2).unwrap().as_str())?)
        } else if let Some(m) = regex!(r"^(.*)&(.*)$").captures(val) {
            Ok(self.parse_value(m.get(1).unwrap().as_str())? & self.parse_value(m.get(2).unwrap().as_str())?)
        } else if let Some(m) = regex!(r"^~(.*)$").captures(val) {
            Ok(!self.parse_value(m.get(1).unwrap().as_str())? & 0xFFFFFFFF)
        } else if let Some(m) = regex!(r"^(.*)\+(.*)$").captures(val) {
            Ok(self.parse_value(m.get(1).unwrap().as_str())? + self.parse_value(m.get(2).unwrap().as_str())?)
        } else if let Some(m) = regex!(r"^(.*)-(.*)$").captures(val) {
            Ok(self.parse_value(m.get(1).unwrap().as_str())? - self.parse_value(m.get(2).unwrap().as_str())?)
        } else {
            Err(anyhow::anyhow!("Cant parse {val}"))
        }
    }

    /// Resolve a peripheral's register-block base address from the C header
    /// defines.
    ///
    /// `data/peripheral_bases.yaml` lists the defines to try, in priority order,
    /// for peripherals whose header name does not follow the `{NAME}_BASE`
    /// convention. The first define that is present and non-zero wins.
    pub fn get_peri_addr(&self, pname: &str) -> Option<u32> {
        let fallback;
        let defines: &[String] = match config::PERIPHERAL_BASES.get(pname) {
            Some(aliases) => aliases,
            None => {
                fallback = [format!("{pname}_BASE"), pname.to_string()];
                &fallback
            }
        };
        defines
            .iter()
            .find_map(|d| self.0.get(d.as_str()).filter(|&&addr| addr != 0))
            .map(|x| u32::try_from(*x).unwrap())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParsedHeader {
    pub cores: Vec<String>,
    pub interrupts: HashMap<String, HashMap<String, u8>>,
    pub defines: HashMap<String, Defines>,
}

impl ParsedHeader {
    /// Get C header defines for this core.
    pub fn get_defines(&self, core_name: &str) -> &Defines {
        let core_name = if !self.interrupts.contains_key(core_name) || !self.defines.contains_key(core_name) {
            "all"
        } else {
            core_name
        };
        self.defines.get(core_name).unwrap()
    }

    /// Get interrupts for this core.
    pub fn get_interrupts(&self, core_name: &str) -> &HashMap<String, u8> {
        let core_name = if !self.interrupts.contains_key(core_name) || !self.defines.contains_key(core_name) {
            "all"
        } else {
            core_name
        };
        self.interrupts.get(core_name).unwrap()
    }

    fn parse(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        Self::parse_inner(path)
    }

    fn parse_inner(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let mut irqs = HashMap::<String, HashMap<String, u8>>::new();
        let mut defines = HashMap::<String, Defines>::new();
        let mut cores = Vec::<String>::new();
        let mut cur_core = "all".to_string();

        // Reused logical-line buffer. C headers wrap long lines with a trailing
        // backslash, so one logical line can span several physical ones.
        let mut line = String::new();
        let f = std::fs::read(&path)?;

        for raw in f.split(|b| b == &b'\n') {
            line.push_str(String::from_utf8_lossy(raw).trim());

            if line.ends_with('\\') {
                line.pop();
                continue;
            }

            // Track which core's `#if defined(... CORE_CM<n> ...)` block we
            // are inside of.
            if line.contains("CORE_CM")
                && let Some(m) = regex!(r"if defined.*CORE_CM(\d+)(PLUS)?").captures(&line)
            {
                cur_core = format!("cm{}", m.get(1).unwrap().as_str());
                if m.get(2).is_some() {
                    cur_core += "p";
                }
                if !cores.contains(&cur_core) {
                    cores.push(cur_core.clone())
                }
            } else if line.contains("else") {
                cur_core = "all".to_string();
                if let Some(m) = regex!(r"else.*CORE_CM(\d+)(PLUS)?").captures(&line) {
                    cur_core = format!("cm{}", m.get(1).unwrap().as_str());
                    if m.get(2).is_some() {
                        cur_core += "p";
                    }
                } else if cores.len() > 1 {
                    // Pick the second core assuming we've already parsed one
                    cur_core = cores[1].clone();
                }

                if !cores.contains(&cur_core) {
                    cores.push(cur_core.clone());
                }
            } else if line.contains("endif") {
                cur_core = "all".to_string();
            }

            if let Some(m) = regex!(r"^([a-zA-Z0-9_]+)_IRQn[ \t]*=[ \t]*(\d+),?[ \t]").captures(&line) {
                irqs.entry(cur_core.clone()).or_default().insert(
                    m.get(1).unwrap().as_str().to_string(),
                    m.get(2).unwrap().as_str().parse().unwrap(),
                );
            }

            if let Some(m) = regex!(r"^#define +([0-9A-Za-z_]+)\(").captures(&line) {
                defines
                    .entry(cur_core.clone())
                    .or_default()
                    .0
                    .insert(m.get(1).unwrap().as_str().to_string(), -1);
            }

            if let Some(m) = regex!(r"^#define +([0-9A-Za-z_]+) +(.*)").captures(&line) {
                let name = m.get(1).unwrap().as_str().trim();
                if name == "FLASH_SIZE" {
                    line.clear();
                    continue;
                }
                let val = m.get(2).unwrap().as_str();
                let val = val.split("/*").next().unwrap().trim();
                let defines_entry = defines.entry(cur_core.clone()).or_default();
                let val = defines_entry
                    .parse_value(val)
                    .with_context(|| format!("Failed to process {:?}", path.as_ref()))?;

                defines_entry.0.insert(name.to_string(), val);
            }

            line.clear();
        }

        if cores.is_empty() {
            cores = vec!["all".to_string()];
        }

        // Every core that was ever current gets an entry (possibly empty) in both
        // maps. `get_defines` and `get_interrupts` use that to tell "unknown core"
        // apart from "core with nothing defined in it".
        for core in cores.iter().map(String::as_str).chain(std::iter::once("all")) {
            irqs.entry(core.to_string()).or_default();
            defines.entry(core.to_string()).or_default();
        }

        for core in &mut cores {
            if core != "all" {
                let all_irqs = irqs.get("all").unwrap().clone();
                irqs.get_mut(core).unwrap().extend(all_irqs);

                let all_defines = defines.get("all").unwrap().clone();
                defines.get_mut(core).unwrap().0.extend(all_defines.0);
            }
        }

        Ok(Self {
            cores,
            interrupts: irqs,
            defines,
        })
    }
}
