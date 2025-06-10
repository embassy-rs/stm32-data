use std::collections::{HashMap, HashSet};

use log::*;

use crate::chips::ChipGroup;
use crate::normalize_peris::normalize_peri_name;
use crate::regex;
use crate::util::RegexMap;

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
pub struct ChipInterrupts {
    // (nvic name, nvic version) => [cursed unparsed interrupt string]
    irqs: HashMap<(String, String), Vec<String>>,
}

impl ChipInterrupts {
    pub fn parse() -> anyhow::Result<Self> {
        let mut irqs = HashMap::new();

        let mut files: Vec<_> = glob::glob("sources/cubedb/mcu/IP/NVIC*_Modes.xml")?
            .map(Result::unwrap)
            .filter(|file| !file.to_string_lossy().contains("STM32MP1"))
            .collect();
        files.sort();

        for f in files {
            trace!("parsing {f:?}");
            let file = std::fs::read_to_string(&f)?;
            let parsed: xml::Ip = quick_xml::de::from_str(&file)?;

            let strings: Vec<_> = parsed
                .ref_parameters
                .into_iter()
                .filter(|param| param.name == "IRQn")
                .flat_map(|param| param.possible_values)
                // F3 can remap USB IRQs, ignore them.
                .filter(|irq| !parsed.version.starts_with("STM32F3") || !irq.comment.contains("remap"))
                .map(|irq| irq.value)
                .collect();

            irqs.insert((parsed.name, parsed.version), strings);
        }

        Ok(Self { irqs })
    }

    pub(crate) fn process(
        &self,
        core: &mut stm32_data_serde::chip::Core,
        chip_name: &str,
        h: &crate::header::ParsedHeader,
        group: &ChipGroup,
    ) -> anyhow::Result<()> {
        trace!("parsing interrupts for chip {} core {}", chip_name, core.name);

        // =================== Populate nvic_priority_bits
        // With the current data sources, this value is always either 2 or 4, and never resolves to None
        let header_defines = h.get_defines(&core.name);
        core.nvic_priority_bits = header_defines.0.get("__NVIC_PRIO_BITS").map(|bits| *bits as u8);

        // =================== Populate core interrupts
        let mut header_irqs = h.get_interrupts(&core.name).clone();
        // F100xE MISC_REMAP remaps some DMA IRQs, so ST decided to give two names
        // to the same IRQ number.
        if chip_name.starts_with("STM32F100") {
            header_irqs.remove("DMA2_Channel4_5");
        }
        core.interrupts = header_irqs
            .iter()
            .map(|(k, v)| stm32_data_serde::chip::core::Interrupt {
                name: k.clone(),
                number: *v,
            })
            .collect();
        core.interrupts.sort_unstable_by_key(|x| x.number);

        // =================== Populate peripheral interrupts
        let core_name = &core.name;
        let want_nvic_name = pick_nvic(chip_name, core_name);
        let chip_nvic = group
            .ips
            .values()
            .find(|x| x.name == want_nvic_name)
            .ok_or_else(|| {
                format!(
                    "couldn't find nvic. chip_name={chip_name} core_name={core_name} want_nvic_name={want_nvic_name}"
                )
            })
            .unwrap();
        let nvic_strings = match self.irqs.get(&(chip_nvic.name.clone(), chip_nvic.version.clone())) {
            Some(strings) => strings,
            None => return Err(anyhow::anyhow!("Failed to find NVIC strings for chip {chip_name}")),
        };

        // peripheral -> signal -> interrupts
        let mut chip_signals = HashMap::<String, HashMap<String, HashSet<String>>>::new();

        let exists_irq: HashSet<String> = core.interrupts.iter().map(|i| i.name.clone()).collect();

        for i in &exists_irq {
            trace!("  irq in header: {i}");
        }

        for nvic_string in nvic_strings {
            trace!("  irq={nvic_string:?}");
            let parts = {
                let mut iter = nvic_string.split(':');
                let parts = [(); 5].map(|_| iter.next().unwrap());
                assert!(iter.next().is_none());
                parts
            };

            let mut name = parts[0].strip_suffix("_IRQn").unwrap().to_string();

            // Fix typo in STM32Lxx and L083 devices
            let contains_rng = || parts[2..].iter().flat_map(|x| x.split(',')).any(|x| x == "RNG");
            if name == "AES_RNG_LPUART1" && !contains_rng() {
                name = "AES_LPUART1".to_string()
            }

            // More typos
            let name = name.replace("USAR11", "USART11");
            trace!("    name={name}");

            // Skip interrupts that don't exist.
            // This is needed because NVIC files are shared between many chips.
            static EQUIVALENT_IRQS: &[(&str, &[&str])] = &[
                ("HASH_RNG", &["RNG"]),
                ("USB_HP_CAN_TX", &["CAN_TX"]),
                ("USB_LP_CAN_RX0", &["CAN_RX0"]),
                ("TIM6_DAC", &["TIM6", "DAC"]),
                ("DMA1_Ch4_7_DMA2_Ch1_5_DMAMUX_OVR", &["DMA1_Ch4_7_DMAMUX_OVR"]),
            ];
            let mut header_name = name.clone();
            if !exists_irq.contains(&name) {
                let &(_, eq_irqs) = EQUIVALENT_IRQS
                    .iter()
                    .find(|(irq, _)| irq == &name)
                    .unwrap_or(&("", &[]));
                let Some(new_name) = eq_irqs.iter().find(|i| exists_irq.contains(**i)) else {
                    trace!("    irq missing in C header, ignoring");
                    continue;
                };
                header_name = new_name.to_string();
            }

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
            if chip_nvic.version == "STM32F100E" && name == "DMA2_Channel4_5" {
                continue;
            }
            //not supported
            if name == "LSECSSD" {
                continue;
            }

            let mut interrupt_signals = HashSet::<(String, String)>::new();
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
            .contains(&name.as_str())
            {
                // pass
            } else if flags
                .iter()
                .any(|flag| ["DMA", "DMAL0", "DMAF0", "DMAL0_DMAMUX", "DMAF0_DMAMUX"].contains(flag))
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
                        interrupt_signals.insert((dma.to_string(), format!("CH{ch}")));
                    }
                }
                assert!(dmas_iter.next().is_none());
                assert!(chans_iter.next().is_none());
            } else if name == "DMAMUX1" || name == "DMAMUX1_S" || name == "DMAMUX_OVR" || name == "DMAMUX1_OVR" {
                interrupt_signals.insert(("DMAMUX1".to_string(), "OVR".to_string()));
            } else if name == "DMAMUX2_OVR" {
                interrupt_signals.insert(("DMAMUX2".to_string(), "OVR".to_string()));
            } else if flags.contains(&"DMAMUX") {
                panic!("should've been handled above");
            } else if flags.contains(&"EXTI") {
                for signal in parts[2].split(',') {
                    interrupt_signals.insert(("EXTI".to_string(), signal.to_string()));
                }
            } else if name == "FLASH" {
                interrupt_signals.insert(("FLASH".to_string(), "GLOBAL".to_string()));
            } else if name == "CRS" {
                interrupt_signals.insert(("RCC".to_string(), "CRS".to_string()));
            } else if name == "RCC" {
                interrupt_signals.insert(("RCC".to_string(), "GLOBAL".to_string()));
            } else if name == "RNG_CRYP" {
                interrupt_signals.insert(("RNG".to_string(), "GLOBAL".to_string()));
                interrupt_signals.insert(("CRYP".to_string(), "GLOBAL".to_string()));
            } else if name == "WWDG_IWDG" {
                interrupt_signals.insert(("WWDG".to_string(), "GLOBAL".to_string()));
                interrupt_signals.insert(("IWDG".to_string(), "GLOBAL".to_string()));
            } else if name == "RCC_AUDIOSYNC" {
                // ignore
            } else {
                if parts[2].is_empty() {
                    trace!("    skipping because parts[2].is_empty()");
                    continue;
                }

                let peri_names: Vec<_> = parts[2]
                    .split(',')
                    .map(|x| if x == "USB_DRD_FS" { "USB" } else { x })
                    .map(|x| if x == "XPI1" { "XSPI1" } else { x })
                    .map(|x| if x == "XPI2" { "XSPI2" } else { x })
                    .map(ToString::to_string)
                    .collect();

                trace!("    peri_names: {peri_names:?}");

                let name2 = {
                    if name == "USBWakeUp" || name == "USBWakeUp_RMP" {
                        "USB_WKUP"
                    } else {
                        name.strip_suffix("_S").unwrap_or(&name)
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

                // Parse IRQ interrupt_signals from the IRQ name.
                for part in tokenize_name(name2) {
                    trace!("    part={part}");

                    let part = {
                        if part == "TAMPER" {
                            "TAMP".to_string()
                        } else {
                            part
                        }
                    };

                    if part == "LSECSS" {
                        interrupt_signals.insert(("RCC".to_string(), "LSECSS".to_string()));
                    } else if part == "CSS" {
                        interrupt_signals.insert(("RCC".to_string(), "CSS".to_string()));
                    } else if part == "LSE" {
                        interrupt_signals.insert(("RCC".to_string(), "LSE".to_string()));
                    } else if part == "CRS" {
                        interrupt_signals.insert(("RCC".to_string(), "CRS".to_string()));
                    } else {
                        let pp = match_peris(&peri_names, &part);
                        trace!("    part={part}, pp={pp:?}");
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

                    // If we have no interrupt_signals for the peri, assume it's "global" so assign it all known ones
                    if ss.is_empty() {
                        if p.starts_with("COMP") {
                            ss = vec!["WKUP".to_string()];
                        } else {
                            ss = known.clone();
                        }
                    }

                    for s in ss {
                        if !known.contains(&s.clone()) {
                            panic!(
                                "Unknown signal {s} for peri {p} chip {chip_name}, known={known:?}, parts={parts:?}"
                            );
                        }
                        trace!("    signal: {} {}", p, s);
                        interrupt_signals.insert((p.clone(), s));
                    }
                }
            }

            for (p, s) in interrupt_signals {
                let p = normalize_peri_name(&p).to_string();
                let signals = chip_signals.entry(p).or_default();
                let irqs = signals.entry(s).or_default();
                irqs.insert(header_name.clone());
            }
        }

        for p in &mut core.peripherals {
            if let Some(signals) = chip_signals.get(&p.name) {
                let mut all_irqs: Vec<stm32_data_serde::chip::core::peripheral::Interrupt> = Vec::new();

                // remove duplicates
                let globals = signals.get("GLOBAL").cloned().unwrap_or_default();
                for (signal, irqs) in signals {
                    let mut irqs = irqs.clone();

                    // If there's a duplicate irqs in a signal other than "global", keep the non-global one.
                    if irqs.len() != 1 && signal != "GLOBAL" {
                        irqs.retain(|irq| !globals.contains(irq));
                    }

                    // If there's still duplicate irqs, keep the one that doesn't match the peri name.
                    if irqs.len() != 1 && signal != "GLOBAL" {
                        irqs.retain(|irq| irq != &p.name);
                    }

                    if irqs.len() != 1 {
                        panic!(
                            "dup irqs on chip {:?} nvic {:?} peri {} signal {}: {:?}",
                            chip_name, chip_nvic.version, p.name, signal, irqs
                        );
                    }

                    for irq in irqs {
                        all_irqs.push(stm32_data_serde::chip::core::peripheral::Interrupt {
                            signal: signal.clone(),
                            interrupt: irq,
                        })
                    }
                }

                all_irqs.sort_by_key(|x| (x.signal.clone(), x.interrupt.clone()));
                all_irqs.dedup_by_key(|x| (x.signal.clone(), x.interrupt.clone()));

                p.interrupts = all_irqs;
            }
        }
        Ok(())
    }
}

fn tokenize_name(name: &str) -> Vec<String> {
    // Treat IRQ names are "tokens" separated by `_`, except some tokens
    // contain `_` themselves, such as `C1_RX`.
    let r =
        regex!(r"(SPDIF_RX|EP\d+_(IN|OUT)|OTG_FS|OTG_HS|USB_DRD_FS|USB_FS|C1_RX|C1_TX|C2_RX|C2_TX|[A-Z0-9]+(_\d+)*)_*");
    let name = name.to_ascii_uppercase();

    r.captures_iter(&name)
        .map(|cap| cap.get(1).unwrap().as_str().to_string())
        .collect()
}

fn match_peris(peris: &[String], name: &str) -> Vec<String> {
    const PERI_OVERRIDE: &[(&str, &[&str])] = &[
        ("USB_FS", &["USB"]),
        ("USB_DRD_FS", &["USB"]),
        ("OTG_HS", &["USB_OTG_HS"]),
        ("OTG_FS", &["USB_OTG_FS"]),
        ("USB", &["USB_DRD_FS"]),
        ("USB_DRD_FS", &["USB"]),
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
        ("I3C", &["ER", "EV", "WKUP"]),
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
        ("PWR", &["S3WU", "WKUP", "PVD"]),
        ("GTZC", &["GLOBAL", "ILA"]),
        ("WWDG", &["GLOBAL", "RST"]),
        ("USB_OTG_FS", &["GLOBAL", "EP1_OUT", "EP1_IN", "WKUP"]),
        ("USB_OTG_HS", &["GLOBAL", "EP1_OUT", "EP1_IN", "WKUP", "USB"]),
        ("USB", &["LP", "HP", "WKUP"]),
        ("GPU2D", &["ER"]),
        ("SAI", &["A", "B"]),
        ("ADF", &["FLT0"]),
        ("RAMECC", &["ECC"]),
    ];

    for (prefix, signals) in IRQ_SIGNALS_MAP {
        if peri.starts_with(prefix) {
            return signals.iter().map(ToString::to_string).collect();
        }
    }
    vec!["GLOBAL".to_string()]
}

static PICK_NVIC: RegexMap<&str> = RegexMap::new(&[
    // Exception 1: Multicore: NVIC1 is the first core, NVIC2 is the second. We have to pick the right one.
    ("STM32H7(45|47|55|57).*:cm7", "NVIC1"),
    ("STM32H7(45|47|55|57).*:cm4", "NVIC2"),
    ("STM32WL5.*:cm4", "NVIC1"),
    ("STM32WL5.*:cm0p", "NVIC2"),
    // Exception 2: TrustZone: NVIC1 is Secure mode, NVIC2 is NonSecure mode. For now, we pick the NonSecure one.
    ("STM32(L5|U5|H5[2367]|WBA5[245]|WBA6[2345]).*", "NVIC2"),
    // Exception 3: NVICs are split for "bootloader" and "application", not sure what that means?
    ("STM32H7[RS].*", "NVIC2"),
    // catch-all: Most chips have a single NVIC, named "NVIC"
    (".*", "NVIC"),
]);

fn pick_nvic(chip_name: &str, core_name: &str) -> String {
    PICK_NVIC.must_get(&format!("{chip_name}:{core_name}")).to_string()
}
