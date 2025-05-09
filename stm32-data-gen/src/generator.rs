use std::collections::hash_map::Entry;
use std::collections::{BTreeSet, HashMap, HashSet};

use gpio_af::pin_sort_key;
use perimap::PERIMAP;
use regex::Regex;
use stm32_data_serde::chip::core::peripheral::Pin;
use util::RegexMap;

use super::*;
use crate::chips::{Chip, ChipGroup};
use crate::gpio_af::parse_signal_name;
use crate::normalize_peris::normalize_peri_name;

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

#[allow(clippy::too_many_arguments)]
fn process_group(
    group: ChipGroup,
    headers: &header::Headers,
    af: &gpio_af::Af,
    chip_interrupts: &interrupts::ChipInterrupts,
    peripheral_to_clock: &rcc::ParsedRccs,
    dma_channels: &dma::DmaChannels,
    chips: &HashMap<String, Chip>,
    docs: &docs::Docs,
) -> Result<(), anyhow::Error> {
    let chip_name = group.chip_names[0].clone();
    let rcc_kind = group.ips.values().find(|x| x.name == "RCC").unwrap().version.clone();
    let rcc_block = *PERIMAP
        .get(&format!("{chip_name}:RCC:{rcc_kind}"))
        .unwrap_or_else(|| panic!("could not get rcc for {}", &chip_name));
    let h = headers
        .get_for_chip(&chip_name)
        .unwrap_or_else(|| panic!("could not get header for {}", &chip_name));
    let chip_af = &group.ips.values().find(|x| x.name == "GPIO").unwrap().version;
    let chip_af = chip_af.strip_suffix("_gpio_v1_0").unwrap();
    let chip_af = af.0.get(chip_af);

    let cores: anyhow::Result<Vec<_>> = group
        .xml
        .cores
        .iter()
        .map(|long_core_name| {
            process_core(
                long_core_name,
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
    let cores = cores?;

    for chip_name in &group.chip_names {
        process_chip(chips, chip_name, h, docs, &group, &cores)?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_core(
    long_core_name: &str,
    h: &header::ParsedHeader,
    chip_name: &str,
    group: &ChipGroup,
    chip_interrupts: &interrupts::ChipInterrupts,
    peripheral_to_clock: &rcc::ParsedRccs,
    rcc_block: (&str, &str, &str),
    chip_af: Option<&HashMap<String, Vec<stm32_data_serde::chip::core::peripheral::Pin>>>,
    dma_channels: &dma::DmaChannels,
) -> anyhow::Result<stm32_data_serde::chip::Core> {
    let core_name = create_short_core_name(long_core_name);
    let defines = h.get_defines(&core_name);

    let mut peripherals =
        create_peripherals_for_chip(chip_name, group, peripheral_to_clock, rcc_block, chip_af, defines);

    apply_family_extras(group, &mut peripherals);

    for p in peripherals.values_mut() {
        // sort and dedup pins, put the ones with AF number first, so we keep them
        p.pins
            .sort_by_key(|x| (x.pin.clone(), x.signal.clone(), x.af.is_none()));
        p.pins.dedup_by_key(|x| (x.pin.clone(), x.signal.clone()));
    }

    let mut peripherals: Vec<_> = peripherals.into_values().collect();
    peripherals.sort_by_key(|x| x.name.clone());

    let dmas = collect_dma_instances(group, dma_channels);
    let dma_channels = extract_relevant_dma_channels(&peripherals, &dmas, chip_name);
    associate_peripherals_dma_channels(&mut peripherals, dmas, &dma_channels);

    let mut pins: Vec<_> = group
        .pins
        .keys()
        .map(|x| x.replace("_C", ""))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|name| stm32_data_serde::chip::core::Pin { name })
        .collect();

    pins.sort_by_key(|p| pin_sort_key(&p.name));

    let mut core = stm32_data_serde::chip::Core {
        name: core_name.clone(),
        peripherals,
        nvic_priority_bits: None,
        interrupts: vec![],
        dma_channels,
        pins,
    };

    chip_interrupts.process(&mut core, chip_name, h, group)?;

    Ok(core)
}

/// Constructs a map of peripherals for a given chip.
///
/// This function processes the provided `group` and extracts relevant peripheral information
/// for the specified `chip_name`.
/// It fills in addresses from `headers`, registers from their YAML definitions via `PERIMAP`,
/// RCC configurations from `peripheral_to_clock` and `rcc_block`, and alternate function pins
/// from `chip_af`.
///
/// Returns a `HashMap` where each key is the name of a peripheral, and each value is a `Peripheral`
/// struct containing the peripheral's configuration details.
fn create_peripherals_for_chip(
    chip_name: &str,
    group: &ChipGroup,
    peripheral_to_clock: &rcc::ParsedRccs,
    rcc_block: (&str, &str, &str),
    chip_af: Option<&HashMap<String, Vec<Pin>>>,
    defines: &header::Defines,
) -> HashMap<String, stm32_data_serde::chip::core::Peripheral> {
    let peri_kinds = create_peripheral_map(chip_name, group, defines);
    let periph_pins = extract_pins_from_chip_group(group);
    let mut peripherals = HashMap::new();
    for (pname, pkind) in peri_kinds {
        // We cannot add this to FAKE peripherals because we need the pins
        if pname.starts_with("I2S") {
            continue;
        }

        let addr = resolve_peri_addr(chip_name, &pname, defines);
        let Some(address) = addr else { continue };

        let registers = if let Some(&block) = PERIMAP.get(&format!("{chip_name}:{pname}:{pkind}")) {
            Some(stm32_data_serde::chip::core::peripheral::Registers {
                kind: block.0.to_string(),
                version: block.1.to_string(),
                block: block.2.to_string(),
            })
        } else {
            None
        };

        let rcc = if let Some(mut rcc_info) = peripheral_to_clock.match_peri_clock(rcc_block.1, &pname) {
            if let Some(stop_mode_info) = low_power::peripheral_stop_mode_info(chip_name, &pname) {
                rcc_info.stop_mode = stop_mode_info;
            }
            Some(rcc_info)
        } else {
            None
        };

        let mut pins = merge_afs_into_core_pins(chip_name, chip_af, &periph_pins, &pname);
        pins.append(&mut merge_i2s_into_spi_pins(chip_name, chip_af, &periph_pins, &pname));

        let p = stm32_data_serde::chip::core::Peripheral {
            name: pname.clone(),
            address,
            registers,
            rcc,
            interrupts: Vec::new(),
            dma_channels: Vec::new(),
            pins,
        };

        peripherals.insert(p.name.clone(), p);
    }
    peripherals
}

/// Create a short core name from a long name.
///
/// Parse a string like "ARM Cortex-M4" or "ARM Cortex-M7 secure" and return a
/// string like "cm4" or "cm7s". The input string is expected to contain the
/// string "Cortex-M" followed by a number, optionally followed by the string
/// "+" or " secure".
///
/// FIXME: "secure" does not appear the XML files in that attribute.
fn create_short_core_name(d: &str) -> String {
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

/// Return a HashMap of peripheral names to their kind (name:version).
///
/// Some peripherals have different names in the xml files and the header files. We handle these cases here by
/// normalizing the peripheral names. The function needs to take care of the different cases where a peripheral is not
/// present in the xml files but is present in the header files, and vice versa.
fn create_peripheral_map(chip_name: &str, group: &ChipGroup, defines: &header::Defines) -> HashMap<String, String> {
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

    // The STM32H7S / STM32H7R use the FDCAN v1 peripheral instead of the one used in the rest of the H7 family.
    let h7_non_rs_re = Regex::new(r"STM32H7[0-9AB].*").unwrap();

    if !fdcans.is_empty() {
        if h7_non_rs_re.is_match(chip_name) {
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
    peri_kinds
}

/// Get pins for each peripheral based on the chip xmls.
///
/// It's not the full info we would want (stuff like AFIO info which comes from
/// the GPIO xml), but we actually need to use it because of the F1 line
/// which doesn't include non-remappable peripherals in the GPIO xml and some
/// weird edge cases like STM32F030C6 (see [merge_periph_pins_info]).
///
/// The function returns a HashMap of peripheral name to Vec of Pins.
/// The Pins only contain the pin and signal name, and no AF information
/// (because we don't have it here).
/// The Vec of Pins is sorted by pin name and deduplicated.
fn extract_pins_from_chip_group(group: &ChipGroup) -> HashMap<String, Vec<Pin>> {
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
        pins.sort_by_key(|p| pin_sort_key(&p.pin));
        pins.dedup();
    }
    periph_pins
}

/// Merge core xml info with GPIO xml info for the given peripheral.
///
/// The GPIO XMLs contain information about AFs for each pin, but the Core XMLs do not.
/// The core XMLs, on the other hand, contain information about available signals on each pin,
/// which the GPIO XMLs do not.
///
/// This function takes the core pins from `periph_pins` and the AFs from `chip_af`
/// for the peripheral `pname`, merges the two using [merge_periph_pins_info] and returns
/// the combined `Pin`s.
fn merge_afs_into_core_pins(
    chip_name: &str,
    chip_af: Option<&HashMap<String, Vec<Pin>>>,
    periph_pins: &HashMap<String, Vec<Pin>>,
    pname: &String,
) -> Vec<Pin> {
    if let Some(pins) = periph_pins.get(pname) {
        let mut pins = pins.clone();
        if let Some(af_pins) = chip_af.and_then(|x| x.get(pname)) {
            merge_periph_pins_info(chip_name, pname, &mut pins, af_pins.as_slice());
        }
        return pins;
    }
    Vec::new()
}

/// Merge I2Sx pins into SPIx pins.
///
/// SPIx peripherals have I2Sx pins which are not explicitly listed in the peripheral's pins.
/// This function merges the I2Sx pins from `chip_af` into the SPIx pins of `periph_pins`,
/// with a prefix "I2S_". If the peripheral `pname` does not start with "SPI", it returns imediately.
fn merge_i2s_into_spi_pins(
    chip_name: &str,
    chip_af: Option<&HashMap<String, Vec<Pin>>>,
    periph_pins: &HashMap<String, Vec<Pin>>,
    pname: &str,
) -> Vec<Pin> {
    if !pname.starts_with("SPI") {
        return Vec::new();
    }
    let i2s_name = "I2S".to_owned() + pname.trim_start_matches("SPI");
    // Do we have I2Sx pins in the peripheral pins and I2Sx AFs in the chip?
    if let Some(i2s_pins) = periph_pins.get(&i2s_name) {
        let mut i2s_pins = i2s_pins.clone();
        if let Some(af_pins) = chip_af.and_then(|x| x.get(&i2s_name)) {
            merge_periph_pins_info(chip_name, &i2s_name, &mut i2s_pins, af_pins.as_slice());
        }
        for p in &mut i2s_pins {
            p.signal = "I2S_".to_owned() + &p.signal;
        }
        return i2s_pins;
    }
    Vec::new()
}

/// Merge AF information from GPIO file into peripheral pins.
///
/// `core_pins` is modified in-place and updated with AF information from `af_pins`.
/// Also does some chip-specific adjustments, so we need `chip_name` and `periph_name`.
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

    // convert to hashmap
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

/// Resolve the address of a peripheral.
///
/// In the case of FDCANRAM, the address can be different depending on the chip. The STM32H7 (not RS) has a single
/// message RAM shared between all FDCANs, while other chips have one message RAM per FDCAN.
fn resolve_peri_addr(chip_name: &str, pname: &str, defines: &header::Defines) -> Option<u32> {
    let addr = if let Some(cap) = regex!(r"^FDCANRAM(?P<idx>[0-9]+)$").captures(pname) {
        defines.get_peri_addr("FDCANRAM").map(|addr| {
            let h7_non_rs_re = Regex::new(r"STM32H7[0-9AB].*").unwrap();
            if h7_non_rs_re.is_match(chip_name) {
                addr
            } else {
                let idx = cap["idx"].parse::<u32>().unwrap();
                // FIXME: this offset should not be hardcoded, but I think
                // it appears in no data sources (only in RMs)
                addr + (idx - 1) * 0x350
            }
        })
    } else {
        defines.get_peri_addr(pname)
    };
    addr
}

/// Merge family-specific YAML overlays into the peripheral map.
///
/// Loads `data/extra/family/{family}.yaml` if it exists, which may contain:
/// - `peripherals`: extra or corrective peripheral entries
/// - `pin_cleanup`: rules to modify pin names
///
/// Parameters:
/// - `group`: ChipGroup context (family/package) for filtering
/// - `peripherals`: mutable map of peripheral name → `Peripheral` to modify
fn apply_family_extras(group: &ChipGroup, peripherals: &mut HashMap<String, stm32_data_serde::chip::core::Peripheral>) {
    if let Ok(extra_f) = std::fs::read(format!("data/extra/family/{}.yaml", group.family)) {
        #[derive(serde::Deserialize)]
        struct PinCleanup {
            strip_suffix: String,
            exclude_peripherals: Vec<String>,
        }
        #[derive(serde::Deserialize)]
        struct Extra {
            peripherals: Option<Vec<stm32_data_serde::chip::core::Peripheral>>,
            pin_cleanup: Option<PinCleanup>,
        }

        let extra: Extra = serde_yaml::from_slice(&extra_f).unwrap();

        // merge extra peripherals
        if let Some(extras) = extra.peripherals {
            for mut p in extras {
                // filter out pins that may not exist in this package.
                p.pins.retain(|p| group.pins.contains_key(&p.pin));

                if let Some(peripheral) = peripherals.get_mut(&p.name) {
                    // Modify the generated peripheral
                    peripheral.pins.append(&mut p.pins);
                } else if p.address != 0 {
                    // Only insert the peripheral if the address is not the default
                    peripherals.insert(p.name.clone(), p);
                }
            }
        }

        // apply pin_cleanup rules
        if let Some(clean) = extra.pin_cleanup {
            for (name, peri) in peripherals.iter_mut() {
                // skip excluded peripherals
                if clean.exclude_peripherals.iter().any(|ex| name.starts_with(ex)) {
                    continue;
                }
                for pin in &mut peri.pins {
                    if let Some(stripped) = pin.pin.strip_suffix(&clean.strip_suffix) {
                        pin.pin = stripped.to_string();
                    }
                }
            }
        }
    }
}

/// Collect and sort all DMA IP instances available on the current chip.
///
/// Iterates over the parsed MCU IP definitions (`group.ips`), matching each IP’s `version`
/// and `instance_name` against the pre-built `dma_channels` map.
/// Searches first by the DMA name (e.g. "BDMA"), then by the DMA name and instance
/// (e.g. "STM32H7RS_dma3_Cube:GPDMA1"). Returns a `Vec<(ip_name, instance_name, ChipDma)>`.
fn collect_dma_instances<'a>(
    group: &ChipGroup,
    dma_channels: &'a dma::DmaChannels,
) -> Vec<(String, String, &'a dma::ChipDma)> {
    // Collect DMA versions in the chip
    let mut dmas: Vec<_> = group
        .ips
        .values()
        .filter_map(|ip| {
            let version = &ip.version;
            let instance = &ip.instance_name;
            dma_channels
                .0
                .get(version)
                .or_else(|| dma_channels.0.get(&format!("{version}:{instance}")))
                .map(|dma| (ip.name.clone(), instance.clone(), dma))
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
    dmas
}

/// Filters and returns a list of DMA channels for the current chip.
///
/// It takes the DMA instances from `dmas`, which also occur in `peripherals`.
/// Of these, only the first channels corresponding to the number of channels
/// in this chip family are then saved.
///
/// E.g. on an STM32F030CC (IP version “STM32F091_dma_v1_1”), DMA2 is removed because
/// only DMA1 exists; on an STM32G431CB only the first 6 channels of each DMA block
/// are kept per the hardware limit.
///
/// It returns `Vec<DmaChannels>` with the valid DMA channel definitions that
/// can actually be used on this specific device.
fn extract_relevant_dma_channels(
    peripherals: &Vec<stm32_data_serde::chip::core::Peripheral>,
    dmas: &Vec<(String, String, &dma::ChipDma)>,
    chip_name: &str,
) -> Vec<stm32_data_serde::chip::core::DmaChannels> {
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
    dma_channels
}

/// Associates each peripheral with its available DMA channels.
///
/// This is done by retrieving its mappings from `dmas`, filtering out
/// channels not present in `dma_channels`, sorting them, and assigning
/// the sorted list to the peripheral’s `dma_channels` field.
///
/// This determines which DMA channels can be used for data transfers for each peripheral.
/// Modifies each `Peripheral` in-place, setting `peripheral.dma_channels`.
fn associate_peripherals_dma_channels(
    peripherals: &mut Vec<stm32_data_serde::chip::core::Peripheral>,
    dmas: Vec<(String, String, &dma::ChipDma)>,
    dma_channels: &Vec<stm32_data_serde::chip::core::DmaChannels>,
) {
    let have_chs: HashSet<_> = dma_channels.iter().map(|ch| ch.name.clone()).collect();

    // Process peripheral - DMA channel associations
    for p in peripherals {
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
        family: group.family.clone(),
        line: group.line.clone(),
        die: group.die.clone(),
        device_id: u16::from_str_radix(&group.die[3..], 16).unwrap(),
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
