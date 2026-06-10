use std::collections::HashMap;
use std::sync::LazyLock;

use super::*;
use crate::util::new_regex_set;

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
        #[serde(rename = "Flash")]
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
        (regex!("^(STM32......).x$"), "$1"),
        (regex!("^(STM32N6....).xQ$"), "$1"),
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

pub fn parse_groups(filter: &Option<String>) -> Result<(HashMap<String, Chip>, Vec<ChipGroup>), anyhow::Error> {
    // XMLs group together chips that are identical except flash/ram size.
    // For example STM32L471Z(E-G)Jx.xml is STM32L471ZEJx, STM32L471ZGJx.
    // However they do NOT group together identical chips with different package.

    // We want exactly the opposite: group all packages of a chip together, but
    // NOT group equal-except-memory-size chips together. Yay.

    // We first read all XMLs, and fold together all packages. We don't expand
    // flash/ram sizes yet, we want to do it as late as possible to avoid duplicate
    // work so that generation is faster.

    let mut chips = HashMap::<String, (Chip, usize)>::new();
    let mut chip_groups = Vec::<(ChipGroup, xml::Mcu)>::new();

    let mut files: Vec<_> = glob::glob("sources/cubedb/mcu/STM32*.xml")?
        .map(Result::unwrap)
        .collect();
    files.sort();

    for f in files {
        if let Some(filter) = filter
            && let Some(file_name) = f.file_name()
            && !file_name.to_ascii_lowercase().to_string_lossy().starts_with(filter)
        {
            continue;
        }

        parse_group(f, &mut chips, &mut chip_groups)?;
    }

    for (chip_name, (_, group_idx)) in &chips {
        chip_groups[*group_idx].0.chip_names.push(chip_name.clone());
    }
    Ok((
        chips.into_iter().map(|(k, (v, _))| (k, v)).collect(),
        chip_groups.into_iter().map(|(group, _)| group).collect(),
    ))
}

static NOPELIST: LazyLock<regex::RegexSet> = LazyLock::new(|| {
    new_regex_set([
        // Not supported, not planned unless someone wants to do it.
        "STM32MP.*",
        // "STM32N6.*",
        "STM32G41[14].*",
        "STM32G4.*xZ",
        "STM32WL3.*",
        // Does not exist in ST website. No datasheet, no RM.
        "STM32GBK.*",
        "STM32L485.*",
        // STM32WxM modules. These are based on a chip that's supported on its own,
        // not sure why we want a separate target for it.
        "STM32WL5M.*",
        "STM32WB1M.*",
        "STM32WB3M.*",
        "STM32WB5M.*",
        "STM32WBA5M.*",
    ])
});

fn parse_group(
    f: std::path::PathBuf,
    chips: &mut HashMap<String, (Chip, usize)>,
    chip_groups: &mut Vec<(ChipGroup, xml::Mcu)>,
) -> anyhow::Result<()> {
    let ff = f.file_name().unwrap().to_string_lossy();

    if NOPELIST.is_match(ff.split('.').next().unwrap()) {
        return Ok(());
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

    let _package_rams = {
        if parsed.rams.len() == 1 {
            vec![parsed.rams[0]; package_names.len()]
        } else {
            parsed.rams.clone()
        }
    };
    let _package_flashes = {
        if parsed.flashs.len() == 1 {
            vec![parsed.flashs[0]; package_names.len()]
        } else {
            parsed.flashs.clone()
        }
    };

    let group_idx = package_names.iter().find_map(|package_name| {
        let chip_name = chip_name_from_package_name(package_name);
        chips.get(&chip_name).map(|(_, group_idx)| *group_idx)
    });

    let group_idx = group_idx.unwrap_or_else(|| {
        let group_idx = chip_groups.len();
        chip_groups.push((
            ChipGroup {
                chip_names: Vec::new(),
                cores: parsed.cores.clone(),
                headers: Vec::new(),
                ips: HashMap::new(),
                pins: HashMap::new(),
                family: parsed.family.clone(),
                line: parsed.line.clone(),
                die: parsed.die.clone(),
                gpio_af: None,
            },
            parsed.clone(),
        ));
        group_idx
    });

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

    for (_package_i, package_name) in package_names.iter().enumerate() {
        chips
            .entry(chip_name_from_package_name(package_name))
            .or_insert_with(|| (Chip { packages: Vec::new() }, group_idx))
            .0
            .packages
            .push(stm32_data_serde::chip::Package {
                name: package_name.clone(),
                package: parsed.package.clone(),
                pins: package_pins.clone(),
            });
    }

    // Some packages have some peripherals removed because the package had to
    // remove GPIOs useful for that peripheral. So we merge all peripherals from all packages.
    let group = &mut chip_groups[group_idx];
    for ip in parsed.ips {
        group.0.ips.insert(ip.instance_name.clone(), ip);
    }

    merge_pins(&mut group.0.pins, parsed.pins.into_iter());

    Ok(())
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
