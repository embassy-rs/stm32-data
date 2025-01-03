use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use perimap::PERIMAP;
use stm32_data_serde::chip::core::peripheral::Pin;
use util::RegexMap;

use super::*;
use crate::gpio_af::parse_signal_name;
use crate::normalize_peris::normalize_peri_name;

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
    packages: Vec<stm32_data_serde::chip::Package>,
}

pub struct ChipGroup {
    chip_names: Vec<String>,
    xml: xml::Mcu,
    pub ips: HashMap<String, xml::Ip>,
    pins: HashMap<String, xml::Pin>,
    family: Option<String>,
    line: Option<String>,
    die: Option<String>,
}

fn chip_name_from_package_name(x: &str) -> String {
    let regexes = [
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
        (regex!("^(STM32WBA....).x$"), "$1"),
        (regex!("^(STM32......).x$"), "$1"),
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

fn corename(d: &str) -> String {
    let m = regex!(r".*Cortex-M(\d+)(\+?)\s*(.*)").captures(d).unwrap();
    let cm = m.get(1).unwrap().as_str();
    let p = if m.get(2).unwrap().as_str() == "+" { "p" } else { "" };
    let s = if m.get(3).unwrap().as_str() == "secure" {
        "s"
    } else {
        ""
    };
    format!("cm{cm}{p}{s}")
}

fn merge_periph_pins_info(
    chip_name: &str,
    periph_name: &str,
    core_pins: &mut [stm32_data_serde::chip::core::peripheral::Pin],
    af_pins: &[stm32_data_serde::chip::core::peripheral::Pin],
) {
    if chip_name.contains("STM32F1") {
        // TODO: actually handle the F1 AFIO information when it will be extracted
        return;
    }

    // covert to hashmap
    let af_pins: HashMap<(&str, &str), Option<u8>> = af_pins
        .iter()
        .map(|v| ((v.pin.as_str(), v.signal.as_str()), v.af))
        .collect();

    for pin in &mut core_pins[..] {
        let af = af_pins.get(&(&pin.pin, &pin.signal)).copied().flatten();

        // try to look for a signal with another name
        let af = af.or_else(|| {
            if pin.signal == "CTS" {
                // for some godforsaken reason UART4's and UART5's CTS are called CTS_NSS in the GPIO xml
                // so try to match with these
                af_pins.get(&(pin.pin.as_str(), "CTS_NSS")).copied().flatten()
            } else if chip_name.starts_with("STM32F0") && periph_name == "I2C1" {
                // it appears that for __some__ STM32 MCUs there is no AFIO specified in GPIO file
                // (notably - STM32F030C6 with it's I2C1 on PF6 and PF7)
                // but the peripheral can actually be mapped to different pins
                // this breaks embassy's model, so we pretend that it's AF 0
                // Reference Manual states that there's no GPIOF_AFR register
                // but according to Cube-generated core it's OK to write to AFIO reg, it seems to be ignored
                // TODO: are there any more signals that have this "feature"
                Some(0)
            } else {
                None
            }
        });

        if let Some(af) = af {
            pin.af = Some(af);
        }
    }

    // apply some renames
    if chip_name.starts_with("STM32C0") || chip_name.starts_with("STM32G0") {
        for pin in &mut core_pins[..] {
            if pin.signal == "MCO" {
                pin.signal = "MCO_1".to_string()
            }
        }
    }
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

static NOPELIST: &[&str] = &[
    // Not supported, not planned unless someone wants to do it.
    "STM32MP",
    // Does not exist in ST website. No datasheet, no RM.
    "STM32GBK",
    "STM32L485",
    // STM32WxM modules. These are based on a chip that's supported on its own,
    // not sure why we want a separate target for it.
    "STM32WL5M",
    "STM32WB1M",
    "STM32WB3M",
    "STM32WB5M",
];

fn parse_group(
    f: std::path::PathBuf,
    chips: &mut HashMap<String, Chip>,
    chip_groups: &mut Vec<ChipGroup>,
) -> anyhow::Result<()> {
    let ff = f.file_name().unwrap().to_string_lossy();

    for nope in NOPELIST {
        if ff.contains(nope) {
            return Ok(());
        }
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
            family: None,
            line: None,
            die: None,
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

    // Some packages have some peripehrals removed because the package had to
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

#[allow(clippy::too_many_arguments)]
fn process_group(
    mut group: ChipGroup,
    headers: &header::Headers,
    af: &gpio_af::Af,
    chip_interrupts: &interrupts::ChipInterrupts,
    peripheral_to_clock: &rcc::ParsedRccs,
    dma_channels: &dma::DmaChannels,
    chips: &HashMap<String, Chip>,
    docs: &docs::Docs,
) -> Result<(), anyhow::Error> {
    let chip_name = group.chip_names[0].clone();
    group.family = Some(group.xml.family.clone());
    group.line = Some(group.xml.line.clone());
    group.die = Some(group.xml.die.clone());
    let rcc_kind = group.ips.values().find(|x| x.name == "RCC").unwrap().version.clone();
    let rcc_block = PERIMAP
        .get(&format!("{chip_name}:RCC:{rcc_kind}"))
        .unwrap_or_else(|| panic!("could not get rcc for {}", &chip_name))
        .clone();
    let h = headers
        .get_for_chip(&chip_name)
        .unwrap_or_else(|| panic!("could not get header for {}", &chip_name));
    let chip_af = &group.ips.values().find(|x| x.name == "GPIO").unwrap().version;
    let chip_af = chip_af.strip_suffix("_gpio_v1_0").unwrap();
    let chip_af = af.0.get(chip_af);

    let cores: Vec<_> = group
        .xml
        .cores
        .iter()
        .map(|core_xml| {
            process_core(
                core_xml,
                h,
                &chip_name,
                &group,
                chip_interrupts,
                peripheral_to_clock,
                rcc_block,
                chip_af,
                dma_channels,
            )
        })
        .collect();

    for chip_name in &group.chip_names {
        process_chip(chips, chip_name, h, docs, &group, &cores)?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_core(
    core_xml: &str,
    h: &header::ParsedHeader,
    chip_name: &str,
    group: &ChipGroup,
    chip_interrupts: &interrupts::ChipInterrupts,
    peripheral_to_clock: &rcc::ParsedRccs,
    rcc_block: (&str, &str, &str),
    chip_af: Option<&HashMap<String, Vec<stm32_data_serde::chip::core::peripheral::Pin>>>,
    dma_channels: &dma::DmaChannels,
) -> stm32_data_serde::chip::Core {
    let core_name = corename(core_xml);
    let defines = h.get_defines(&core_name);

    let mut peri_kinds = HashMap::new();
    for ip in group.ips.values() {
        let pname = normalize_peri_name(&ip.instance_name);
        let pkind = format!("{}:{}", ip.name, ip.version);
        let pkind = pkind.strip_suffix("_Cube").unwrap_or(&pkind);

        const FAKE_PERIPHERALS: &[&str] = &[
            // These are real peripherals but with special handling
            "NVIC",
            "GPIO",
            "DMA",
            // IRTIM is just TIM16+TIM17
            "IRTIM",
            // We add this as ghost peri
            "SYS",
            "ADC_COMMON",
            "ADC1_COMMON",
            "ADC12_COMMON",
            "ADC123_COMMON",
            "ADC3_COMMON",
            "ADC4_COMMON",
            "ADC34_COMMON",
            "ADC345_COMMON",
            // These are software libraries
            "FREERTOS",
            "PDM2PCM",
            "FATFS",
            "LIBJPEG",
            "MBEDTLS",
            "LWIP",
            "USB_HOST",
            "USB_DEVICE",
            "GUI_INTERFACE",
            "TRACER_EMB",
            "TOUCHSENSING",
        ];

        if FAKE_PERIPHERALS.contains(&pname) {
            continue;
        }

        peri_kinds.insert(pname.to_string(), pkind.to_string());
    }
    const GHOST_PERIS: &[&str] = &[
        "GPIOA",
        "GPIOB",
        "GPIOC",
        "GPIOD",
        "GPIOE",
        "GPIOF",
        "GPIOG",
        "GPIOH",
        "GPIOI",
        "GPIOJ",
        "GPIOK",
        "GPIOL",
        "GPIOM",
        "GPION",
        "GPIOO",
        "GPIOP",
        "GPIOQ",
        "GPIOR",
        "GPIOS",
        "GPIOT",
        "DMA1",
        "DMA2",
        "BDMA",
        "DMAMUX",
        "DMAMUX1",
        "DMAMUX2",
        "SBS",
        "SYSCFG",
        "EXTI",
        "FLASH",
        "DBGMCU",
        "CRS",
        "PWR",
        "AFIO",
        "BKP",
        "USBRAM",
        "VREFINTCAL",
        "UID",
        "HSEM",
        "ADC1_COMMON",
        "ADC12_COMMON",
        "ADC123_COMMON",
        "ADC3_COMMON",
        "ADC4_COMMON",
        "ADC34_COMMON",
        "ADC345_COMMON",
        "VREFBUF",
    ];
    for pname in GHOST_PERIS {
        let normalized_pname = normalize_peri_name(pname);
        if let Entry::Vacant(entry) = peri_kinds.entry(normalized_pname.to_string()) {
            if defines.get_peri_addr(pname).is_some() {
                entry.insert("unknown".to_string());
            }
        }
    }
    if peri_kinds.contains_key("BDMA1") {
        peri_kinds.remove("BDMA");
    }
    let fdcans = peri_kinds
        .keys()
        .filter_map(|pname| {
            regex!(r"^FDCAN(?P<idx>[0-9]+)$")
                .captures(pname)
                .map(|cap| cap["idx"].to_string())
        })
        .collect::<Vec<_>>();
    if !fdcans.is_empty() {
        if chip_name.starts_with("STM32H7") {
            // H7 has one message RAM shared between FDCANs
            peri_kinds
                .entry("FDCANRAM".to_string())
                .or_insert("unknown".to_string());
        } else {
            // Other chips with FDCANs have separate message RAM per module
            for fdcan in fdcans {
                peri_kinds
                    .entry(format!("FDCANRAM{}", fdcan))
                    .or_insert("unknown".to_string());
            }
        }
    }
    // get possible used GPIOs for each peripheral from the chip xml
    // it's not the full info we would want (stuff like AFIO info which comes from GPIO xml),
    //   but we actually need to use it because of F1 line
    //       which doesn't include non-remappable peripherals in GPIO xml
    //   and some weird edge cases like STM32F030C6 (see merge_periph_pins_info)
    let mut periph_pins = HashMap::<_, Vec<_>>::new();
    for (pin_name, pin) in &group.pins {
        for signal in &pin.signals {
            let signal = &signal.name;
            // TODO: What are those signals (well, GPIO is clear) Which peripheral do they belong to?
            if ["GPIO", "CEC", "AUDIOCLK", "VDDTCXO"].contains(&signal.as_str()) || signal.contains("EXTI") {
                continue;
            }
            let Some((signal_peri, signal_name)) = parse_signal_name(signal) else {
                continue;
            };
            periph_pins.entry(signal_peri.to_string()).or_default().push(
                stm32_data_serde::chip::core::peripheral::Pin {
                    pin: pin_name.clone(),
                    signal: signal_name.to_string(),
                    af: None,
                },
            );
        }
    }
    for pins in periph_pins.values_mut() {
        pins.sort();
        pins.dedup();
    }
    let mut peripherals = HashMap::new();
    for (pname, pkind) in peri_kinds {
        // We cannot add this to FAKE peripherals because we need the pins
        if pname.starts_with("I2S") {
            continue;
        }

        let addr = if let Some(cap) = regex!(r"^FDCANRAM(?P<idx>[0-9]+)$").captures(&pname) {
            defines.get_peri_addr("FDCANRAM").map(|addr| {
                if chip_name.starts_with("STM32H7") {
                    addr
                } else {
                    let idx = cap["idx"].parse::<u32>().unwrap();
                    // FIXME: this offset should not be hardcoded, but I think
                    // it appears in no data sources (only in RMs)
                    addr + (idx - 1) * 0x350
                }
            })
        } else {
            defines.get_peri_addr(&pname)
        };

        let Some(addr) = addr else { continue };

        let mut p = stm32_data_serde::chip::core::Peripheral {
            name: pname.clone(),
            address: addr,
            registers: None,
            rcc: None,
            interrupts: Vec::new(),
            dma_channels: Vec::new(),
            pins: Vec::new(),
        };

        if let Some(&block) = PERIMAP.get(&format!("{chip_name}:{pname}:{pkind}")) {
            p.registers = Some(stm32_data_serde::chip::core::peripheral::Registers {
                kind: block.0.to_string(),
                version: block.1.to_string(),
                block: block.2.to_string(),
            });
        }

        if let Some(rcc_info) = peripheral_to_clock.match_peri_clock(rcc_block.1, &pname) {
            p.rcc = Some(rcc_info);
        }
        if let Some(pins) = periph_pins.get_mut(&pname) {
            // merge the core xml info with GPIO xml info to hopefully get the full picture
            // if the peripheral does not exist in the GPIO xml (one of the notable one is ADC)
            //   it probably doesn't need any AFIO writes to work
            if let Some(af_pins) = chip_af.and_then(|x| x.get(&pname)) {
                merge_periph_pins_info(chip_name, &pname, pins, af_pins.as_slice());
            }
            p.pins = pins.clone();
        }

        let i2s_name = if pname.starts_with("SPI") {
            "I2S".to_owned() + pname.trim_start_matches("SPI")
        } else {
            "".to_owned()
        };

        if let Some(i2s_pins) = periph_pins.get_mut(&i2s_name) {
            // merge the core xml info with GPIO xml info to hopefully get the full picture
            // if the peripheral does not exist in the GPIO xml (one of the notable one is ADC)
            //   it probably doesn't need any AFIO writes to work
            if let Some(af_pins) = chip_af.and_then(|x| x.get(&i2s_name)) {
                merge_periph_pins_info(chip_name, &i2s_name, i2s_pins, af_pins.as_slice());
            }

            p.pins.extend(i2s_pins.iter().map(|p| Pin {
                pin: p.pin.clone(),
                signal: "I2S_".to_owned() + &p.signal,
                af: p.af,
            }));
        }

        // H7 has some _C pin variants (e.g. PC2 and PC2_C). Digital stuff should always be in the non-C pin.
        // cubedb puts it either in both, or in the -C pin only! (in chips where the package has only the -C pin)
        // so we fix that up here.
        if !pname.starts_with("ADC") && !pname.starts_with("DAC") && !pname.starts_with("COMP") {
            for pin in &mut p.pins {
                if let Some(p) = pin.pin.strip_suffix("_C") {
                    pin.pin = p.to_string();
                }
            }
        }

        // sort pins to avoid diff for c pins
        // put the ones with AF number first, so we keep them.
        p.pins
            .sort_by_key(|x| (x.pin.clone(), x.signal.clone(), x.af.is_none()));
        p.pins.dedup_by_key(|x| (x.pin.clone(), x.signal.clone()));

        peripherals.insert(p.name.clone(), p);
    }
    if let Ok(extra_f) = std::fs::read(format!("data/extra/family/{}.yaml", group.family.as_ref().unwrap())) {
        #[derive(serde::Deserialize)]
        struct Extra {
            peripherals: Vec<stm32_data_serde::chip::core::Peripheral>,
        }

        let extra: Extra = serde_yaml::from_slice(&extra_f).unwrap();
        for mut p in extra.peripherals {
            if let Some(peripheral) = peripherals.get_mut(&p.name) {
                // Modify the generated peripheral
                peripheral.pins.append(&mut p.pins);
            } else if p.address != 0 {
                // Only insert the peripheral if the address is not the default
                peripherals.insert(p.name.clone(), p);
            }
        }
    }

    let mut peripherals: Vec<_> = peripherals.into_values().collect();
    peripherals.sort_by_key(|x| x.name.clone());
    // Collect DMA versions in the chip
    let mut dmas: Vec<_> = group
        .ips
        .values()
        .filter_map(|ip| {
            let version = &ip.version;
            let instance = &ip.instance_name;
            if let Some(dma) = dma_channels
                .0
                .get(version)
                .or_else(|| dma_channels.0.get(&format!("{version}:{instance}")))
            {
                Some((ip.name.clone(), instance.clone(), dma))
            } else {
                None
            }
        })
        .collect();
    dmas.sort_by_key(|(name, instance, _)| {
        (
            match name.as_str() {
                "DMA" => 1,
                "BDMA" => 2,
                "BDMA1" => 3,
                "BDMA2" => 4,
                "GPDMA" => 5,
                "HPDMA" => 6,
                _ => 0,
            },
            instance.clone(),
        )
    });

    // The dma_channels[xx] is generic for multiple chips. The current chip may have less DMAs,
    // so we have to filter it.
    static DMA_CHANNEL_COUNTS: RegexMap<usize> = RegexMap::new(&[
        ("STM32F0[37]0.*:DMA1", 5),
        ("STM32G4[34]1.*:DMA1", 6),
        ("STM32G4[34]1.*:DMA2", 6),
    ]);
    let have_peris: HashSet<_> = peripherals.iter().map(|p| p.name.clone()).collect();
    let dma_channels = dmas
        .iter()
        .flat_map(|(_, _, dma)| dma.channels.clone())
        .filter(|ch| have_peris.contains(&ch.dma))
        .filter(|ch| {
            DMA_CHANNEL_COUNTS
                .get(&format!("{}:{}", chip_name, ch.dma))
                .map_or_else(|| true, |&v| usize::from(ch.channel) < v)
        })
        .collect::<Vec<_>>();
    let have_chs: HashSet<_> = dma_channels.iter().map(|ch| ch.name.clone()).collect();

    // Process peripheral - DMA channel associations
    for p in &mut peripherals {
        let mut chs = Vec::new();
        for (_, _, dma) in &dmas {
            if let Some(peri_chs) = dma.peripherals.get(&p.name) {
                chs.extend(
                    peri_chs
                        .iter()
                        .filter(|ch| match &ch.channel {
                            None => true,
                            Some(channel) => have_chs.contains(channel),
                        })
                        .cloned(),
                );
            }
        }
        chs.sort_by_key(|ch| (ch.channel.clone(), ch.dmamux.clone(), ch.request));
        p.dma_channels = chs;
    }

    let mut core = stm32_data_serde::chip::Core {
        name: core_name.clone(),
        peripherals,
        nvic_priority_bits: None,
        interrupts: vec![],
        dma_channels,
    };

    chip_interrupts.process(&mut core, chip_name, h, group);

    core
}

fn process_chip(
    chips: &HashMap<String, Chip>,
    chip_name: &str,
    _h: &header::ParsedHeader,
    docs: &docs::Docs,
    group: &ChipGroup,
    cores: &[stm32_data_serde::chip::Core],
) -> Result<(), anyhow::Error> {
    let chip = chips.get(chip_name).unwrap();
    let docs = docs.documents_for(chip_name);
    let chip = stm32_data_serde::Chip {
        name: chip_name.to_string(),
        family: group.family.clone().unwrap(),
        line: group.line.clone().unwrap(),
        die: group.die.clone().unwrap(),
        device_id: u16::from_str_radix(&group.die.as_ref().unwrap()[3..], 16).unwrap(),
        packages: chip.packages.clone(),
        memory: memory::get(chip_name),
        docs,
        cores: cores.to_vec(),
    };

    crate::check::check(&chip);

    let dump = serde_json::to_string_pretty(&chip)?;
    std::fs::write(format!("build/data/chips/{chip_name}.json"), dump)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn dump_all_chips(
    chip_groups: Vec<ChipGroup>,
    headers: header::Headers,
    af: gpio_af::Af,
    chip_interrupts: interrupts::ChipInterrupts,
    peripheral_to_clock: rcc::ParsedRccs,
    dma_channels: dma::DmaChannels,
    chips: std::collections::HashMap<String, Chip>,
    docs: docs::Docs,
) -> Result<(), anyhow::Error> {
    std::fs::create_dir_all("build/data/chips")?;

    #[cfg(feature = "rayon")]
    {
        use rayon::prelude::*;

        chip_groups.into_par_iter().try_for_each(|group| {
            process_group(
                group,
                &headers,
                &af,
                &chip_interrupts,
                &peripheral_to_clock,
                &dma_channels,
                &chips,
                &docs,
            )
        })
    }
    #[cfg(not(feature = "rayon"))]
    {
        chip_groups.into_iter().try_for_each(|group| {
            process_group(
                group,
                &headers,
                &af,
                &chip_interrupts,
                &peripheral_to_clock,
                &dma_channels,
                &chips,
                &docs,
            )
        })
    }
}
