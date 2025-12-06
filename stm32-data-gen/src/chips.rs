use std::collections::HashMap;

use util::RegexSet;

use super::*;

mod xml {
    use serde::Deserialize;

    #[derive(Clone, Debug, Deserialize, PartialEq)]
    pub struct Mcu {
        #[serde(rename = "Family")]
        pub family: String,
        #[serde(rename = "Line")]
        pub line: String,
        #[serde(rename = "Die")]
        pub die: String,
        #[serde(rename = "RefName")]
        pub ref_name: String,
        #[serde(rename = "Package")]
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
        #[serde(rename = "Name")]
        pub name: String,
        #[serde(rename = "Position")]
        pub position: String,
        #[serde(rename = "Signal", default)]
        pub signals: Vec<PinSignal>,
    }

    #[derive(Clone, Debug, Deserialize, PartialEq)]
    pub struct PinSignal {
        #[serde(rename = "Name")]
        pub name: String,
    }

    #[derive(Clone, Debug, Deserialize, PartialEq)]
    pub struct Ip {
        #[serde(rename = "InstanceName")]
        pub instance_name: String,
        #[serde(rename = "Name")]
        pub name: String,
        #[serde(rename = "Version")]
        pub version: String,
    }
}

pub struct Chip {
    #[allow(dead_code)]
    flash: u32,
    #[allow(dead_code)]
    ram: u32,
    group_idx: usize,
    pub packages: Vec<stm32_data_serde::chip::Package>,
}

pub struct ChipGroup {
    pub chip_names: Vec<String>,
    pub xml: xml::Mcu,
    pub ips: HashMap<String, xml::Ip>,
    pub pins: HashMap<String, xml::Pin>,
    pub family: String,
    pub line: String,
    pub die: String,
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

pub fn parse_groups() -> Result<(HashMap<String, Chip>, Vec<ChipGroup>), anyhow::Error> {
    // XMLs group together chips that are identical except flash/ram size.
    // For example STM32L471Z(E-G)Jx.xml is STM32L471ZEJx, STM32L471ZGJx.
    // However they do NOT group together identical chips with different package.

    // We want exactly the opposite: group all packages of a chip together, but
    // NOT group equal-except-memory-size chips together. Yay.

    // We first read all XMLs, and fold together all packages. We don't expand
    // flash/ram sizes yet, we want to do it as late as possible to avoid duplicate
    // work so that generation is faster.

    let mut chips = HashMap::<String, Chip>::new();
    let mut chip_groups = Vec::new();

    let mut files: Vec<_> = glob::glob("sources/cubedb/mcu/STM32*.xml")?
        .map(Result::unwrap)
        .collect();
    files.sort();

    for f in files {
        parse_group(f, &mut chips, &mut chip_groups)?;
    }

    for (chip_name, chip) in &chips {
        chip_groups[chip.group_idx].chip_names.push(chip_name.clone());
    }
    Ok((chips, chip_groups))
}

static NOPELIST: RegexSet = RegexSet::new(&[
    // Not supported, not planned unless someone wants to do it.
    "STM32MP.*",
    // TODO, PRs welcome :)
    "STM32U3.*",
    // "STM32N6.*",
    "STM32G41[14].*",
    "STM32G4.*xZ",
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
]);

fn parse_group(
    f: std::path::PathBuf,
    chips: &mut HashMap<String, Chip>,
    chip_groups: &mut Vec<ChipGroup>,
) -> anyhow::Result<()> {
    let ff = f.file_name().unwrap().to_string_lossy();

    if NOPELIST.contains(ff.split('.').next().unwrap()) {
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

    let package_rams = {
        if parsed.rams.len() == 1 {
            vec![parsed.rams[0]; package_names.len()]
        } else {
            parsed.rams.clone()
        }
    };
    let package_flashes = {
        if parsed.flashs.len() == 1 {
            vec![parsed.flashs[0]; package_names.len()]
        } else {
            parsed.flashs.clone()
        }
    };

    let group_idx = package_names.iter().find_map(|package_name| {
        let chip_name = chip_name_from_package_name(package_name);
        chips.get(&chip_name).map(|chip| chip.group_idx)
    });

    let group_idx = group_idx.unwrap_or_else(|| {
        let group_idx = chip_groups.len();
        chip_groups.push(ChipGroup {
            chip_names: Vec::new(),
            xml: parsed.clone(),
            ips: HashMap::new(),
            pins: HashMap::new(),
            family: parsed.family.clone(),
            line: parsed.line.clone(),
            die: parsed.die.clone(),
        });
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

    for (package_i, package_name) in package_names.iter().enumerate() {
        let chip_name = chip_name_from_package_name(package_name);
        if !chips.contains_key(&chip_name) {
            chips.insert(
                chip_name.clone(),
                Chip {
                    flash: package_flashes[package_i],
                    ram: package_rams[package_i],
                    group_idx,
                    packages: Vec::new(),
                },
            );
        }
        chips
            .get_mut(&chip_name)
            .unwrap()
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
        group.ips.insert(ip.instance_name.clone(), ip);
    }
    for pin in parsed.pins {
        if let Some(pin_name) = gpio_af::clean_pin(&pin.name) {
            group
                .pins
                .entry(pin_name)
                .and_modify(|p| {
                    // merge signals.
                    p.signals.extend_from_slice(&pin.signals);
                    p.signals.dedup();
                })
                .or_insert(pin);
        }
    }

    Ok(())
}
