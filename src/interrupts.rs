use std::collections::{HashMap, HashSet};

use crate::regex;

mod xml {
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct Ip {
        #[serde(rename = "Name")]
        pub name: String,
        #[serde(rename = "Version")]
        pub version: String,
        #[serde(rename = "RefParameter")]
        pub ref_parameters: Vec<RefParameter>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct RefParameter {
        #[serde(rename = "Name")]
        pub name: String,
        #[serde(rename = "PossibleValue", default)]
        pub possible_values: Vec<PossibleValue>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct PossibleValue {
        #[serde(rename = "Comment")]
        pub comment: String,
        #[serde(rename = "Value")]
        pub value: String,
    }
}

#[derive(Debug)]
pub struct ChipInterrupts(
    pub HashMap<(String, String), HashMap<String, Vec<stm32_data_serde::chip::core::peripheral::Interrupt>>>,
);

impl ChipInterrupts {
    pub fn parse() -> anyhow::Result<Self> {
        let mut chip_interrupts = HashMap::new();

        let mut files: Vec<_> = glob::glob("sources/cubedb/mcu/IP/NVIC*_Modes.xml")?
            .map(Result::unwrap)
            .filter(|file| !file.to_string_lossy().contains("STM32MP1"))
            .collect();
        files.sort();

        for f in files {
            let mut irqs = HashMap::<String, _>::new();
            let file = std::fs::read_to_string(f)?;
            let parsed: xml::Ip = quick_xml::de::from_str(&file)?;
            for irq in parsed
                .ref_parameters
                .into_iter()
                .filter(|param| param.name == "IRQn")
                .flat_map(|param| param.possible_values)
            {
                let parts = {
                    let mut iter = irq.value.split(':');
                    let parts = [(); 5].map(|_| iter.next().unwrap());
                    assert!(iter.next().is_none());
                    parts
                };

                let name = {
                    let name = parts[0].strip_suffix("_IRQn").unwrap();

                    // Fix typo in STM32Lxx and L083 devices
                    let contains_rng = || parts[2..].iter().flat_map(|x| x.split(',')).any(|x| x == "RNG");
                    if name == "AES_RNG_LPUART1" && !contains_rng() {
                        "AES_LPUART1"
                    } else {
                        name
                    }
                };

                let entry = match irqs.entry(name.to_string()) {
                    std::collections::hash_map::Entry::Occupied(_) => continue,
                    std::collections::hash_map::Entry::Vacant(entry) => entry,
                };

                // Flags.
                // Y
                //   unknown, it's in all of them
                // H3, nHS
                //   ???
                // 2V, 3V, nV, 2V1
                //   unknown, it has to do with the fact the irq is shared among N peripehrals
                // DMA, DMAL0, DMAF0, DMAL0_DMAMUX, DMAF0_DMAMUX
                //   special format for DMA
                // DFSDM
                //   special format for DFSDM
                // EXTI
                //   special format for EXTI
                let flags: Vec<_> = parts[1].split(',').collect();

                // F100xE MISC_REMAP remaps some DMA IRQs, so ST decided to give two names
                // to the same IRQ number.
                if parsed.version == "STM32F100E" && name == "DMA2_Channel4_5" {
                    continue;
                }
                // F3 can remap USB IRQs, ignore them.
                if parsed.version.starts_with("STM32F3") && irq.comment.contains("remap") {
                    continue;
                }
                let mut signals = HashSet::<(String, String)>::new();
                if [
                    "NonMaskableInt",
                    "HardFault",
                    "MemoryManagement",
                    "BusFault",
                    "UsageFault",
                    "SVCall",
                    "DebugMonitor",
                    "PendSV",
                    "SysTick",
                ]
                .contains(&name)
                {
                    // pass
                } else if flags
                    .iter()
                    .map(|flag| ["DMA", "DMAL0", "DMAF0", "DMAL0_DMAMUX", "DMAF0_DMAMUX"].contains(flag))
                    .any(std::convert::identity)
                {
                    let mut dmas_iter = parts[3].split(',');
                    let mut chans_iter = parts[4].split(';');
                    for (dma, chan) in std::iter::zip(&mut dmas_iter, &mut chans_iter) {
                        let range = {
                            let mut ch = chan.split(',');
                            let ch_from: usize = ch.next().unwrap().parse().unwrap();
                            let ch_to = match ch.next() {
                                Some(ch_to) => ch_to.parse().unwrap(),
                                None => ch_from,
                            };
                            assert!(ch.next().is_none());
                            ch_from..=ch_to
                        };
                        for ch in range {
                            signals.insert((dma.to_string(), format!("CH{ch}")));
                        }
                    }
                    assert!(dmas_iter.next().is_none());
                    assert!(chans_iter.next().is_none());
                } else if name == "DMAMUX1" || name == "DMAMUX1_S" || name == "DMAMUX_OVR" || name == "DMAMUX1_OVR" {
                    signals.insert(("DMAMUX1".to_string(), "OVR".to_string()));
                } else if name == "DMAMUX2_OVR" {
                    signals.insert(("DMAMUX2".to_string(), "OVR".to_string()));
                } else if flags.contains(&"DMAMUX") {
                    panic!("should've been handled above");
                } else if flags.contains(&"EXTI") {
                    for signal in parts[2].split(',') {
                        signals.insert(("EXTI".to_string(), signal.to_string()));
                    }
                } else if name == "FLASH" {
                    signals.insert(("FLASH".to_string(), "GLOBAL".to_string()));
                } else if name == "CRS" {
                    signals.insert(("RCC".to_string(), "CRS".to_string()));
                } else if name == "RCC" {
                    signals.insert(("RCC".to_string(), "GLOBAL".to_string()));
                } else {
                    if parts[2].is_empty() {
                        continue;
                    }

                    let peri_names: Vec<_> = parts[2].split(',').map(ToString::to_string).collect();

                    let name2 = {
                        if name == "USBWakeUp" || name == "USBWakeUp_RMP" {
                            "USB_WKUP"
                        } else {
                            name.strip_suffix("_S").unwrap_or(name)
                        }
                    };

                    let mut peri_signals: HashMap<_, _> = peri_names
                        .iter()
                        .map(|name| (name.clone(), Vec::<String>::new()))
                        .collect();

                    let mut curr_peris = Vec::new();
                    if peri_names.len() == 1 {
                        curr_peris = peri_names.clone();
                    }

                    // Parse IRQ signals from the IRQ name.
                    for part in tokenize_name(name2) {
                        let part = {
                            if part == "TAMPER" {
                                "TAMP".to_string()
                            } else {
                                part
                            }
                        };

                        if part == "LSECSS" {
                            signals.insert(("RCC".to_string(), "LSECSS".to_string()));
                        } else if part == "CSS" {
                            signals.insert(("RCC".to_string(), "CSS".to_string()));
                        } else if part == "LSE" {
                            signals.insert(("RCC".to_string(), "LSE".to_string()));
                        } else if part == "CRS" {
                            signals.insert(("RCC".to_string(), "CRS".to_string()));
                        } else {
                            let pp = match_peris(&peri_names, &part);
                            if !pp.is_empty() {
                                curr_peris = pp;
                            } else {
                                assert!(!curr_peris.is_empty());
                                for p in &curr_peris {
                                    peri_signals.entry(p.clone()).or_default().push(part.clone());
                                }
                            }
                        }
                    }

                    for (p, mut ss) in peri_signals.into_iter() {
                        let known = valid_signals(&p);

                        // If we have no signals for the peri, assume it's "global" so assign it all known ones
                        if ss.is_empty() {
                            if p.starts_with("COMP") {
                                ss = vec!["WKUP".to_string()];
                            } else {
                                ss = known.clone();
                            }
                        }

                        for s in ss {
                            if !known.contains(&s.clone()) {
                                panic!("Unknown signal {s} for peri {p}, known={known:?}");
                            }
                            signals.insert((p.clone(), s));
                        }
                    }
                }

                // for (peri, signal) in &signals {
                //     println!("    {peri}:{signal}");
                // }
                entry.insert(signals);
            }

            let mut irqs2 = HashMap::<_, Vec<_>>::new();
            for (name, signals) in irqs {
                for (p, s) in signals {
                    let key = if p == "USB_DRD_FS" {
                        "USB".to_string()
                    } else {
                        p
                    };
                    irqs2
                        .entry(key)
                        .or_default()
                        .push(stm32_data_serde::chip::core::peripheral::Interrupt {
                            signal: s,
                            interrupt: name.clone(),
                        });
                }
            }

            for pirqs in irqs2.values_mut() {
                let mut psirqs = HashMap::<_, Vec<_>>::new();
                for irq in pirqs {
                    psirqs
                        .entry(irq.signal.clone())
                        .or_default()
                        .push(irq.interrupt.clone());
                }
                // for (s, irqs) in psirqs {
                //     if irqs.len() != 1 {
                //         println!("DUPE: {p} {s} {irqs:?}");
                //     }
                // }
            }

            chip_interrupts.insert((parsed.name, parsed.version), irqs2);
        }

        Ok(Self(chip_interrupts))
    }
}

fn tokenize_name(name: &str) -> Vec<String> {
    // Treat IRQ names are "tokens" separated by `_`, except some tokens
    // contain `_` themselves, such as `C1_RX`.
    let r = regex!(r"(SPDIF_RX|EP\d+_(IN|OUT)|OTG_FS|OTG_HS|USB_FS|C1_RX|C1_TX|C2_RX|C2_TX|[A-Z0-9]+(_\d+)*)_*");
    let name = name.to_ascii_uppercase();

    r.captures_iter(&name)
        .map(|cap| cap.get(1).unwrap().as_str().to_string())
        .collect()
}

fn match_peris(peris: &[String], name: &str) -> Vec<String> {
    const PERI_OVERRIDE: &[(&str, &[&str])] = &[
        ("USB_FS", &["USB"]),
        ("OTG_HS", &["USB_OTG_HS"]),
        ("OTG_FS", &["USB_OTG_FS"]),
        ("USB", &["USB_DRD_FS"]),
        ("UCPD1_2", &["UCPD1", "UCPD2"]),
        ("ADC1", &["ADC"]),
        ("CEC", &["HDMI_CEC"]),
        ("SPDIF_RX", &["SPDIFRX1", "SPDIFRX"]),
        ("CAN1", &["CAN"]),
        ("TEMP", &["TEMPSENS"]),
        ("DSI", &["DSIHOST"]),
        ("HRTIM1", &["HRTIM"]),
        ("GTZC", &["GTZC_S"]),
        ("TZIC", &["GTZC_S"]),
    ];

    let peri_override: HashMap<_, _> = PERI_OVERRIDE.iter().copied().collect();

    if let Some(over) = peri_override.get(name) {
        let mut res = Vec::new();
        for p in *over {
            if peris.contains(&p.to_string()) {
                res.push(p.to_string());
            }
        }
        if !res.is_empty() {
            return res;
        }
    }
    let mut name = name;
    let mut res = Vec::new();
    if let Some(m) = regex!(r"^(I2C|[A-Z]+)(\d+(_\d+)*)$").captures(name) {
        name = m.get(1).unwrap().as_str();
        for n in m.get(2).unwrap().as_str().split('_') {
            let p = format!("{name}{n}");
            if !peris.contains(&p) {
                return Vec::new();
            }
            res.push(p);
        }
    } else {
        for p in peris {
            if p == name || { p.starts_with(name) && regex!(r"^\d+$").is_match(p.strip_prefix(name).unwrap_or(p)) } {
                res.push(p.to_string());
            }
        }
    }

    res
}

fn valid_signals(peri: &str) -> Vec<String> {
    const IRQ_SIGNALS_MAP: &[(&str, &[&str])] = &[
        ("CAN", &["TX", "RX0", "RX1", "SCE"]),
        ("FDCAN", &["IT0", "IT1", "CAL"]),
        ("I2C", &["ER", "EV"]),
        ("FMPI2C", &["ER", "EV"]),
        ("TIM", &["BRK", "UP", "TRG", "COM", "CC"]),
        // ("HRTIM", &["Master", "TIMA", "TIMB", "TIMC", "TIMD", "TIME", "TIMF"]),
        ("RTC", &["ALARM", "WKUP", "TAMP", "STAMP", "SSRU"]),
        ("SUBGHZ", &["RADIO"]),
        ("IPCC", &["C1_RX", "C1_TX", "C2_RX", "C2_TX"]),
        (
            "HRTIM",
            &["MASTER", "TIMA", "TIMB", "TIMC", "TIMD", "TIME", "TIMF", "FLT"],
        ),
        ("COMP", &["WKUP", "ACQ"]),
        ("RCC", &["RCC", "CRS"]),
        ("MDIOS", &["GLOBAL", "WKUP"]),
        ("ETH", &["GLOBAL", "WKUP"]),
        ("LTDC", &["GLOBAL", "ER"]),
        (
            "DFSDM",
            &["FLT0", "FLT1", "FLT2", "FLT3", "FLT4", "FLT5", "FLT6", "FLT7"],
        ),
        ("MDF", &["FLT0", "FLT1", "FLT2", "FLT3", "FLT4", "FLT5", "FLT6", "FLT7"]),
        ("PWR", &["S3WU"]),
        ("GTZC", &["GLOBAL", "ILA"]),
        ("WWDG", &["GLOBAL", "RST"]),
        ("USB_OTG_FS", &["GLOBAL", "EP1_OUT", "EP1_IN", "WKUP"]),
        ("USB_OTG_HS", &["GLOBAL", "EP1_OUT", "EP1_IN", "WKUP"]),
        ("USB", &["LP", "HP", "WKUP"]),
    ];

    for (prefix, signals) in IRQ_SIGNALS_MAP {
        if peri.starts_with(prefix) {
            return signals.iter().map(ToString::to_string).collect();
        }
    }
    vec!["GLOBAL".to_string()]
}
