use std::collections::HashMap;

use lazy_regex::regex;

use super::*;
use crate::config;

pub mod xml {
    use serde::Deserialize;

    #[derive(Clone, Debug, Deserialize, PartialEq)]
    pub struct Mcu {
        #[serde(rename = "@Family")]
        pub family: String,
        #[serde(rename = "@Line")]
        pub line: String,
        #[serde(rename = "Die")]
        pub die: String,
        #[serde(rename = "@RefName")]
        pub ref_name: String,
        #[serde(rename = "@Package")]
        pub package: String,
        #[serde(rename = "Core")]
        pub cores: Vec<String>,
        #[serde(rename = "Ram")]
        pub rams: Vec<u32>,
        // Absent in some CubeMX XMLs (e.g. STM32H5E/F 3 MB parts); flash layout comes from memory.rs.
        #[serde(rename = "Flash", default)]
        pub flashs: Vec<u32>,
        #[serde(rename = "IP")]
        pub ips: Vec<Ip>,
        #[serde(rename = "Pin")]
        pub pins: Vec<Pin>,
    }

    #[derive(Clone, Debug, Deserialize, PartialEq)]
    pub struct Pin {
        #[serde(rename = "@Name")]
        pub name: String,
        #[serde(rename = "@Position")]
        pub position: String,
        #[serde(rename = "Signal", default)]
        pub signals: Vec<PinSignal>,
    }

    #[derive(Clone, Debug, Deserialize, PartialEq)]
    pub struct PinSignal {
        #[serde(rename = "@Name")]
        pub name: String,
    }

    #[derive(Clone, Debug, Deserialize, PartialEq)]
    pub struct Ip {
        #[serde(rename = "@InstanceName")]
        pub instance_name: String,
        #[serde(rename = "@Name")]
        pub name: String,
        #[serde(rename = "@Version")]
        pub version: String,
    }
}

#[derive(Debug)]
pub struct Chip {
    pub packages: Vec<stm32_data_serde::chip::Package>,
}

#[derive(Debug)]
pub struct ChipGroup {
    pub chip_names: Vec<String>,
    pub cores: Vec<String>,
    pub headers: Vec<String>,
    pub ips: HashMap<String, xml::Ip>,
    pub pins: HashMap<String, xml::Pin>,
    pub family: String,
    pub line: String,
    pub die: String,
    pub gpio_af: Option<String>,
}

fn chip_name_from_package_name(x: &str) -> String {
    let regexes = [
        (regex!("^(STM32C0....).xN$"), "$1"),
        (regex!("^(STM32L1....).x([AX])$"), "$1-$2"),
        (regex!("^(STM32G0....).xN$"), "$1"),
        (regex!("^(STM32F412..).xP$"), "$1"),
        (regex!("^(STM32L4....).x[PS]$"), "$1"),
        (regex!("^(STM32WB....).x[AE]$"), "$1"),
        (regex!("^(STM32G0....).xN$"), "$1"),
        (regex!("^(STM32L5....).x[PQ]$"), "$1"),
        (regex!("^(STM32L0....).xS$"), "$1"),
        (regex!("^(STM32H7....).x[QH]$"), "$1"),
        (regex!("^(STM32U3....).x[QG]$"), "$1"),
        (regex!("^(STM32U5....).xQ$"), "$1"),
        (regex!("^(STM32H5....).xQ$"), "$1"),
        (regex!("^(STM32WB0....).x$"), "$1"),
        (regex!("^(STM32WBA....).x$"), "$1"),
        (regex!("^(STM32MP15...).x$"), "$1"),
        (regex!("^(STM32......).x$"), "$1"),
        // Secure variants use an xQG package suffix (e.g. STM32N645A0HxQG).
        (regex!("^(STM32N6....).xQ?G?$"), "$1"),
    ];

    regexes
        .iter()
        .find_map(|(a, b)| {
            a.captures(x).map(|cap| {
                let mut res = String::new();
                cap.expand(b, &mut res);
                res
            })
        })
        .unwrap_or_else(|| panic!("bad name: {x}"))
}

/// A parsed CubeMX MCU XML, before it is folded into a [`ChipGroup`].
struct ParsedMcu {
    /// Chip name for each entry of `package_names`.
    chip_names: Vec<String>,
    /// `RefName` with its `(XYZ)` package suffix expanded into separate names.
    package_names: Vec<String>,
    package: String,
    package_pins: Vec<stm32_data_serde::chip::PackagePin>,
    cores: Vec<String>,
    family: String,
    line: String,
    die: String,
    ips: Vec<xml::Ip>,
    pins: Vec<xml::Pin>,
}

pub fn parse_groups(filter: &Option<String>) -> Result<(HashMap<String, Chip>, Vec<ChipGroup>), anyhow::Error> {
    // XMLs group together chips that are identical except flash/ram size.
    // For example STM32L471Z(E-G)Jx.xml is STM32L471ZEJx, STM32L471ZGJx.
    // However they do NOT group together identical chips with different package.

    // We want exactly the opposite: group all packages of a chip together, but
    // NOT group equal-except-memory-size chips together. Yay.

    // We first read all XMLs, and fold together all packages. We don't expand
    // flash/ram sizes yet, we want to do it as late as possible to avoid duplicate
    // work so that generation is faster.

    let mut files: Vec<_> = glob::glob("sources/cubedb/mcu/STM32*.xml")?
        .map(Result::unwrap)
        .collect();
    files.sort();
    files.retain(|f| {
        !matches!(
            (filter, f.file_name()),
            (Some(filter), Some(file_name))
                if !file_name.to_ascii_lowercase().to_string_lossy().starts_with(filter)
        )
    });

    // Parsing the XMLs dominates this phase; only the bookkeeping below needs to
    // be ordered, so keep the parse parallel and the merge sequential.
    #[cfg(feature = "rayon")]
    let parsed = {
        use rayon::prelude::*;
        files
            .par_iter()
            .map(|f| parse_group(f))
            .collect::<anyhow::Result<Vec<_>>>()?
    };
    #[cfg(not(feature = "rayon"))]
    let parsed = files
        .iter()
        .map(|f| parse_group(f))
        .collect::<anyhow::Result<Vec<_>>>()?;

    let mut chips = HashMap::<String, (Chip, usize)>::new();
    let mut chip_groups = Vec::<ChipGroup>::new();
    for parsed in parsed.into_iter().flatten() {
        merge_group(parsed, &mut chips, &mut chip_groups);
    }

    // `chip_names[0]` names the group, so sort instead of relying on `HashMap`
    // iteration order to pick it.
    let mut by_group: Vec<(usize, String)> = chips
        .iter()
        .map(|(name, (_, group_idx))| (*group_idx, name.clone()))
        .collect();
    by_group.sort();
    for (group_idx, name) in by_group {
        chip_groups[group_idx].chip_names.push(name);
    }

    Ok((chips.into_iter().map(|(k, (v, _))| (k, v)).collect(), chip_groups))
}

/// Parse one CubeMX MCU XML, or return `None` if the chip is on the ignore list.
fn parse_group(f: &std::path::Path) -> anyhow::Result<Option<ParsedMcu>> {
    let ff = f.file_name().unwrap().to_string_lossy();

    if config::IGNORE.is_match(ff.split('.').next().unwrap()) {
        return Ok(None);
    }

    let parsed: xml::Mcu = quick_xml::de::from_str(&std::fs::read_to_string(f)?)?;

    let package_names = {
        let name = &parsed.ref_name;
        if !name.contains('(') {
            vec![name.to_string()]
        } else {
            let (prefix, suffix) = name.split_once('(').unwrap();
            let (letters, suffix) = suffix.split_once(')').unwrap();
            letters.split('-').map(|x| format!("{prefix}{x}{suffix}")).collect()
        }
    };
    let chip_names: Vec<String> = package_names
        .iter()
        .map(|name| chip_name_from_package_name(name))
        .collect();

    let mut package_pins: HashMap<String, Vec<String>> = HashMap::new();
    for pin in &parsed.pins {
        package_pins
            .entry(pin.position.clone())
            .or_default()
            .push(gpio_af::clean_pin(&pin.name).unwrap_or_else(|| pin.name.clone()));
    }
    let mut package_pins: Vec<stm32_data_serde::chip::PackagePin> = package_pins
        .into_iter()
        .map(|(position, mut signals)| {
            signals.retain(|s| s != "NC");
            signals.sort();
            stm32_data_serde::chip::PackagePin { position, signals }
        })
        .collect();
    package_pins.sort_by_key(|p| match p.position.parse::<u32>() {
        Ok(n) => (Some(n), None),
        Err(_) => (None, Some(p.position.clone())),
    });

    Ok(Some(ParsedMcu {
        chip_names,
        package_names,
        package: parsed.package,
        package_pins,
        cores: parsed.cores,
        family: parsed.family,
        line: parsed.line,
        die: parsed.die,
        ips: parsed.ips,
        pins: parsed.pins,
    }))
}

/// Fold one parsed XML into `chips` and `chip_groups`.
fn merge_group(parsed: ParsedMcu, chips: &mut HashMap<String, (Chip, usize)>, chip_groups: &mut Vec<ChipGroup>) {
    let ParsedMcu {
        chip_names,
        package_names,
        package,
        package_pins,
        cores,
        family,
        line,
        die,
        ips,
        pins,
    } = parsed;

    let group_idx = chip_names
        .iter()
        .find_map(|chip_name| chips.get(chip_name).map(|(_, group_idx)| *group_idx));

    let group_idx = group_idx.unwrap_or_else(|| {
        chip_groups.push(ChipGroup {
            chip_names: Vec::new(),
            cores,
            headers: Vec::new(),
            ips: HashMap::new(),
            pins: HashMap::new(),
            family,
            line,
            die,
            gpio_af: None,
        });
        chip_groups.len() - 1
    });

    for (chip_name, package_name) in chip_names.iter().zip(&package_names) {
        chips
            .entry(chip_name.clone())
            .or_insert_with(|| (Chip { packages: Vec::new() }, group_idx))
            .0
            .packages
            .push(stm32_data_serde::chip::Package {
                name: package_name.clone(),
                package: package.clone(),
                pins: package_pins.clone(),
            });
    }

    // Some packages have some peripherals removed because the package had to
    // remove GPIOs useful for that peripheral. So we merge all peripherals from all packages.
    let group = &mut chip_groups[group_idx];
    for ip in ips {
        group.ips.insert(ip.instance_name.clone(), ip);
    }

    merge_pins(&mut group.pins, pins.into_iter());
}

pub fn merge_pins(group_pins: &mut HashMap<String, xml::Pin>, pins: impl Iterator<Item = xml::Pin>) {
    for pin in pins {
        if let Some(pin_name) = gpio_af::clean_pin(&pin.name) {
            group_pins
                .entry(pin_name)
                .and_modify(|p| {
                    // merge signals.
                    p.signals.extend_from_slice(&pin.signals);
                    p.signals.dedup();
                })
                .or_insert(pin);
        }
    }
}
