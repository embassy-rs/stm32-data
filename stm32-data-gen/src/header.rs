use std::collections::HashMap;

use crate::regex;

pub struct Headers {
    map: HeaderMap,
    parsed: HeadersParsed,
    regexes: Vec<(regex::Regex, String)>,
}

impl Headers {
    pub fn parse() -> anyhow::Result<Self> {
        let map = HeaderMap::parse()?;
        let parsed = HeadersParsed::parse()?;
        let regexes = parsed
            .0
            .keys()
            .map(|h| {
                let pattern = h.replace('x', ".");
                let regex = regex::Regex::new(&format!("^{pattern}$")).unwrap();
                (regex, h.clone())
            })
            .collect();
        Ok(Self { map, parsed, regexes })
    }

    pub fn get_for_chip(&self, model: &str) -> Option<&ParsedHeader> {
        let model = model.to_ascii_lowercase();
        match self.map.0.get(&model) {
            // if it's in the map, just go
            Some(name) => Some(self.parsed.0.get(name).unwrap()),
            // if not, find it by regex, taking `x` meaning `anything`
            None => {
                let mut results = self
                    .regexes
                    .iter()
                    .filter_map(|(r, name)| if r.is_match(&model) { Some(name) } else { None });
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
        for (mut header, chips) in
            serde_yaml::from_str::<HashMap<String, String>>(&std::fs::read_to_string("data/header_map.yaml")?)?
        {
            header.make_ascii_lowercase();
            for chip in chips.split(',') {
                let chip = chip.trim().to_ascii_lowercase();
                if let Some(old) = res.insert(chip.clone(), header.clone()) {
                    panic!("Duplicate {chip} found! Overwriting {old} with {header}");
                }
            }
        }

        Ok(Self(res))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct HeadersParsed(pub HashMap<String, ParsedHeader>);

impl HeadersParsed {
    pub fn parse() -> anyhow::Result<Self> {
        let files = glob::glob("sources/headers/*.h").unwrap().map(Result::unwrap);

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
    fn parse_value(&self, val: &str) -> i64 {
        let val = val.trim();
        if val.is_empty() {
            0
        } else if let Some(m) = regex!(r"^(0([1-9][0-9]*)(U))").captures(val) {
            m.get(2).unwrap().as_str().parse().unwrap()
        } else if let Some(m) = regex!(r"^((0x[0-9a-fA-F]+|\d+))(|u|ul|U|UL)$").captures(val) {
            let x = m.get(1).unwrap().as_str();
            match x.strip_prefix("0x") {
                Some(x) => i64::from_str_radix(x, 16),
                None => x.parse(),
            }
            .unwrap()
        } else if let Some(m) = regex!(r"^([0-9A-Za-z_]+)$").captures(val) {
            self.0.get(m.get(1).unwrap().as_str()).copied().unwrap_or(0)
        } else if let Some(x) = regex!(r"^\((.*)\)$")
            .captures(val)
            .map(|m| m.get(1).unwrap().as_str())
            .filter(|x| parens_ok(x))
        {
            self.parse_value(x)
        } else if let Some(m) = regex!(r"^\*?\([0-9A-Za-z_]+ *\*?\)(.*)$").captures(val) {
            self.parse_value(m.get(1).unwrap().as_str())
        } else if let Some(m) = regex!(r"^(.*)/(.*)$").captures(val) {
            self.parse_value(m.get(1).unwrap().as_str()) / self.parse_value(m.get(2).unwrap().as_str())
        } else if let Some(m) = regex!(r"^(.*)<<(.*)$").captures(val) {
            self.parse_value(m.get(1).unwrap().as_str()) << self.parse_value(m.get(2).unwrap().as_str()) & 0xFFFFFFFF
        } else if let Some(m) = regex!(r"^(.*)>>(.*)$").captures(val) {
            self.parse_value(m.get(1).unwrap().as_str()) >> self.parse_value(m.get(2).unwrap().as_str())
        } else if let Some(m) = regex!(r"^(.*)\|(.*)$").captures(val) {
            self.parse_value(m.get(1).unwrap().as_str()) | self.parse_value(m.get(2).unwrap().as_str())
        } else if let Some(m) = regex!(r"^(.*)&(.*)$").captures(val) {
            self.parse_value(m.get(1).unwrap().as_str()) & self.parse_value(m.get(2).unwrap().as_str())
        } else if let Some(m) = regex!(r"^~(.*)$").captures(val) {
            !self.parse_value(m.get(1).unwrap().as_str()) & 0xFFFFFFFF
        } else if let Some(m) = regex!(r"^(.*)\+(.*)$").captures(val) {
            self.parse_value(m.get(1).unwrap().as_str()) + self.parse_value(m.get(2).unwrap().as_str())
        } else if let Some(m) = regex!(r"^(.*)-(.*)$").captures(val) {
            self.parse_value(m.get(1).unwrap().as_str()) - self.parse_value(m.get(2).unwrap().as_str())
        } else {
            panic!("can't parse: {val:?}")
        }
    }

    pub fn get_peri_addr(&self, pname: &str) -> Option<u32> {
        const ALT_PERI_DEFINES: &[(&str, &[&str])] = &[
            ("DBGMCU", &["DBGMCU_BASE", "DBG_BASE"]),
            ("QUADSPI", &["QUADSPI_BASE", "QSPI_R", "QSPI_R_BASE", "QSPI_REG_BASE"]),
            ("QUADSPI1", &["QUADSPI1_BASE", "QSPI_R", "QSPI_R_BASE", "QSPI_REG_BASE"]),
            ("FLASH", &["FLASH_R_BASE", "FLASH_REG_BASE"]),
            (
                "ADC_COMMON",
                &["ADC_COMMON", "ADC1_COMMON", "ADC12_COMMON", "ADC123_COMMON"],
            ),
            ("ADC3_COMMON", &["ADC3_COMMON", "ADC4_COMMON", "ADC34_COMMON"]),
            ("CAN", &["CAN_BASE", "CAN1_BASE"]),
            ("FMC", &["FMC_BASE", "FMC_R_BASE"]),
            ("FSMC", &["FSMC_R_BASE"]),
            ("USB", &["USB_BASE", "USB_DRD_BASE", "USB_BASE_NS", "USB_DRD_BASE_NS"]),
            (
                "USBRAM",
                &["USB_PMAADDR", "USB_DRD_PMAADDR", "USB_PMAADDR_NS", "USB_DRD_PMAADDR_NS"],
            ),
            ("FDCANRAM", &["SRAMCAN_BASE", "SRAMCAN_BASE_NS"]),
            ("VREFINTCAL", &["VREFINT_CAL_ADDR_CMSIS"]),
        ];
        let alt_peri_defines: HashMap<_, _> = ALT_PERI_DEFINES.iter().copied().collect();

        let possible_defines: Vec<String> = alt_peri_defines
            .get(pname)
            .map(|x| x.iter().map(ToString::to_string).collect())
            .unwrap_or_else(|| vec![format!("{pname}_BASE"), pname.to_string()]);
        possible_defines
            .into_iter()
            .find_map(|d| self.0.get(&d).filter(|&&addr| addr != 0))
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

    fn parse(f: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let mut irqs = HashMap::<String, HashMap<String, u8>>::new();
        let mut defines = HashMap::<String, Defines>::new();
        let mut cores = Vec::<String>::new();
        let mut cur_core = "all".to_string();

        let mut accum = String::new();
        let f = std::fs::read(f)?;
        for l in f.split(|b| b == &b'\n') {
            let l = String::from_utf8_lossy(l);
            let l = l.trim();
            let l = accum.clone() + l;
            if l.ends_with('\\') {
                accum = l.strip_suffix('\\').unwrap().to_string();
                continue;
            }
            accum = String::new();

            // Scoped by a single core
            if let Some(m) = regex!(r".*if defined.*CORE_CM(\d+)(PLUS)?.*").captures(&l) {
                cur_core = format!("cm{}", m.get(1).unwrap().as_str());
                if m.get(2).is_some() {
                    cur_core += "p";
                }
                if !cores.contains(&cur_core) {
                    cores.push(cur_core.clone())
                }
            } else if regex!(r".*else.*").is_match(&l) {
                cur_core = "all".to_string();
                if let Some(m) = regex!(".*else.*CORE_CM(\\d+)(PLUS)?.*").captures(&l) {
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
            } else if regex!(r".*endif.*").is_match(&l) {
                cur_core = "all".to_string();
            }

            let irq_entry = irqs.entry(cur_core.clone()).or_default();
            let defines_entry = defines.entry(cur_core.clone()).or_default();

            if let Some(m) = regex!(r"^([a-zA-Z0-9_]+)_IRQn += (\d+),? +/\*!< (.*) \*/").captures(&l) {
                irq_entry.insert(
                    m.get(1).unwrap().as_str().to_string(),
                    m.get(2).unwrap().as_str().parse().unwrap(),
                );
            }

            if let Some(m) = regex!(r"^#define +([0-9A-Za-z_]+)\(").captures(&l) {
                defines_entry.0.insert(m.get(1).unwrap().as_str().to_string(), -1);
            }

            if let Some(m) = regex!(r"^#define +([0-9A-Za-z_]+) +(.*)").captures(&l) {
                let name = m.get(1).unwrap().as_str().trim();
                if name == "FLASH_SIZE" {
                    continue;
                }
                let val = m.get(2).unwrap().as_str();
                let val = val.split("/*").next().unwrap().trim();
                let val = defines_entry.parse_value(val);

                defines_entry.0.insert(name.to_string(), val);
            }
        }

        if cores.is_empty() {
            cores = vec!["all".to_string()];
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
