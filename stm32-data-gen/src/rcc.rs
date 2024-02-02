use std::cmp::Ordering;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use anyhow::{anyhow, Ok};
use chiptool::ir::{BlockItemInner, Enum};
use stm32_data_serde::chip::core::peripheral;
use stm32_data_serde::chip::core::peripheral::rcc::{Mux, StopMode};

use crate::regex;
use crate::registers::Registers;

/// Deterministically insert into a `HashMap`
fn insert_into_map<K, V, F>(hash_map: &mut HashMap<K, V>, key: K, value: V, mut compare: F)
where
    K: Eq + Hash,
    F: FnMut(&V, &V) -> Ordering,
{
    match hash_map.entry(key) {
        Entry::Vacant(e) => {
            e.insert(value);
        }
        Entry::Occupied(mut e) => {
            if compare(&value, e.get()) == Ordering::Less {
                e.insert(value);
            }
        }
    };
}

#[derive(Debug)]
pub struct PeripheralToClock(
    HashMap<(String, String, String), HashMap<String, stm32_data_serde::chip::core::peripheral::Rcc>>,
);

impl PeripheralToClock {
    pub fn parse(registers: &Registers) -> anyhow::Result<Self> {
        let mut peripheral_to_clock = HashMap::new();
        let allowed_variants = HashSet::from([
            "DISABLE",
            "SYS",
            "PCLK1",
            "PCLK1_TIM",
            "PCLK2",
            "PCLK2_TIM",
            "PCLK3",
            "PCLK4",
            "PCLK5",
            "PCLK6",
            "PCLK7",
            "HCLK1",
            "HCLK2",
            "HCLK3",
            "HCLK4",
            "HCLK5",
            "HCLK6",
            "HCLK7",
            "PLLI2S1_P",
            "PLLI2S1_Q",
            "PLLI2S1_R",
            "PLLI2S2_P",
            "PLLI2S2_Q",
            "PLLI2S2_R",
            "PLLSAI1_P",
            "PLLSAI1_Q",
            "PLLSAI1_R",
            "PLLSAI2_P",
            "PLLSAI2_Q",
            "PLLSAI2_R",
            "PLL1_P",
            "PLL1_Q",
            "PLL1_R",
            "PLL1_VCO", // used for L0 USB
            "PLL2_P",
            "PLL2_Q",
            "PLL2_R",
            "PLL3_P",
            "PLL3_Q",
            "PLL3_R",
            "HSI",
            "SHSI",
            "HSI48",
            "LSI",
            "CSI",
            "MSI",
            "MSIS",
            "MSIK",
            "HSE",
            "LSE",
            "AUDIOCLK",
            "PER",
            "CLK48",
            // TODO: variants to cleanup
            "AFIF",
            "HSI_HSE",
            "DSI_PHY",
            "HSI_Div488",
            "SAI1_EXTCLK",
            "SAI2_EXTCLK",
            "B_0x0",
            "B_0x1",
            "I2S_CKIN",
            "DAC_HOLD",
            "DAC_HOLD_2",
            "RTCCLK",
            "RTC_WKUP",
            "DSIPHY",
            "ICLK",
            "DCLK",
            "I2S1",
            "I2S2",
            "SAI1",
            "SAI2",
            "HSI256_MSIS1024_MSIS4",
            "HSI256_MSIS1024_MSIK4",
            "HSI256_MSIK1024_MSIS4",
            "HSI256_MSIK1024_MSIK4",
        ]);

        for (rcc_name, ir) in &registers.registers {
            if let Some(rcc_name) = rcc_name.strip_prefix("rcc_") {
                let rcc_enum_map: HashMap<&String, HashMap<&String, (&String, &Enum)>> = {
                    let rcc_blocks = &ir.blocks.get("RCC").unwrap().items;

                    rcc_blocks
                        .iter()
                        .filter_map(|b| match &b.inner {
                            BlockItemInner::Register(register) => register.fieldset.as_ref().map(|f| {
                                let f = ir.fieldsets.get(f).unwrap();
                                (
                                    &b.name,
                                    f.fields
                                        .iter()
                                        .filter_map(|f| {
                                            let enumm_name = f.enumm.as_ref()?;
                                            let enumm = ir.enums.get(enumm_name)?;

                                            Some((&f.name, (enumm_name, enumm)))
                                        })
                                        .collect(),
                                )
                            }),
                            _ => None,
                        })
                        .collect()
                };

                let check_mux = |register: &String, field: &String| -> Result<(), anyhow::Error> {
                    let block_map = match rcc_enum_map.get(register) {
                        Some(block_map) => block_map,
                        _ => return Ok(()),
                    };

                    let (enumm_name, enumm) = match block_map.get(field) {
                        Some(enumm) => enumm,
                        _ => return Ok(()),
                    };

                    for v in &enumm.variants {
                        if let Some(captures) = regex!(r"^([A-Z0-9_]+)_DIV_\d+?$").captures(v.name.as_str()) {
                            let name = captures.get(1).unwrap();

                            if !allowed_variants.contains(name.as_str()) {
                                return Err(anyhow!(
                                    "rcc: prohibited variant name {} in enum {} for rcc_{}",
                                    v.name.as_str(),
                                    enumm_name,
                                    rcc_name
                                ));
                            }
                        } else if !allowed_variants.contains(v.name.as_str()) {
                            return Err(anyhow!(
                                "rcc: prohibited variant name {} in enum {} for rcc_{}",
                                v.name.as_str(),
                                enumm_name,
                                rcc_name
                            ));
                        }
                    }

                    Ok(())
                };

                let mut family_muxes = HashMap::new();
                for (reg, body) in &ir.fieldsets {
                    let key = format!("fieldset/{reg}");
                    if regex!(r"^fieldset/(CCIPR|DCKCFGR)\d?$").captures(&key).is_some() {
                        for field in &body.fields {
                            if let Some(peri) = field.name.strip_suffix("SEL") {
                                check_mux(reg, &field.name)?;

                                insert_into_map(
                                    &mut family_muxes,
                                    peri.to_string(),
                                    Mux {
                                        register: reg.to_ascii_lowercase(),
                                        field: field.name.to_ascii_lowercase(),
                                    },
                                    |mux1: &Mux, mux2: &Mux| mux1.register.cmp(&mux2.register),
                                );
                            }
                        }
                    } else if regex!(r"^fieldset/CFGR\d?$").captures(&key).is_some() {
                        for field in &body.fields {
                            if let Some(peri) = field.name.strip_suffix("SW") {
                                check_mux(reg, &field.name)?;

                                insert_into_map(
                                    &mut family_muxes,
                                    peri.to_string(),
                                    Mux {
                                        register: reg.to_ascii_lowercase(),
                                        field: field.name.to_ascii_lowercase(),
                                    },
                                    |mux1: &Mux, mux2: &Mux| mux1.register.cmp(&mux2.register),
                                );
                            }
                        }
                    } else if regex!(r"^fieldset/D\d?CCIPR$").captures(&key).is_some() {
                        for field in &body.fields {
                            if let Some(peri) = field.name.strip_suffix("SEL") {
                                if family_muxes.get(peri).is_some() && reg != "D1CCIPR" {
                                    continue;
                                }

                                check_mux(reg, &field.name)?;

                                insert_into_map(
                                    &mut family_muxes,
                                    peri.to_string(),
                                    Mux {
                                        register: reg.to_ascii_lowercase(),
                                        field: field.name.to_ascii_lowercase(),
                                    },
                                    |mux1: &Mux, mux2: &Mux| mux1.register.cmp(&mux2.register),
                                );
                            }
                        }
                    }
                }

                let mut family_clocks = HashMap::new();
                for (reg, body) in &ir.fieldsets {
                    let key = format!("fieldset/{reg}");
                    if let Some(m) = regex!(r"^fieldset/((A[PH]B\d?)|GPIO)[LH]?ENR\d?$").captures(&key) {
                        let clock = m.get(1).unwrap().as_str();
                        let clock = match clock {
                            "AHB" => "AHB1",
                            "APB" => "APB1",
                            clock => clock,
                        };
                        for field in &body.fields {
                            if let Some(peri) = field.name.strip_suffix("EN") {
                                let peri = if peri == "RTCAPB" { "RTC" } else { peri };
                                let stop_mode = if peri == "RTC" {
                                    StopMode::Standby
                                } else if peri.starts_with("LP") {
                                    StopMode::Stop2
                                } else {
                                    StopMode::Stop1
                                };

                                // Timers are a bit special, they may have a x2 freq
                                let peri_clock = {
                                    if regex!(r"^TIM\d+$").is_match(peri) {
                                        format!("{clock}_TIM")
                                    } else {
                                        clock.to_string()
                                    }
                                };

                                let mut reset = None;
                                if let Some(rstr) = ir.fieldsets.get(&reg.replace("ENR", "RSTR")) {
                                    if let Some(_field) =
                                        rstr.fields.iter().find(|field| field.name == format!("{peri}RST"))
                                    {
                                        reset = Some(stm32_data_serde::chip::core::peripheral::rcc::Reset {
                                            register: reg.replace("ENR", "RSTR").to_ascii_lowercase(),
                                            field: format!("{peri}RST").to_ascii_lowercase(),
                                        });
                                    }
                                }

                                let mux = family_muxes.get(peri).cloned();

                                match family_clocks.entry(peri.to_string()) {
                                    Entry::Vacant(e) => {
                                        e.insert(peripheral::Rcc {
                                            clock: peri_clock
                                                .to_ascii_lowercase()
                                                .replace("ahb", "hclk")
                                                .replace("apb", "pclk"),
                                            enable: peripheral::rcc::Enable {
                                                register: reg.to_ascii_lowercase(),
                                                field: field.name.to_ascii_lowercase(),
                                            },
                                            stop_mode,
                                            reset,
                                            mux,
                                        });
                                    }
                                    Entry::Occupied(_) => {
                                        return Err(anyhow!("rcc: duplicate clock for {} for rcc_{}", peri, rcc_name));
                                    }
                                };
                            }
                        }
                    }
                }
                peripheral_to_clock.insert(
                    ("rcc".to_string(), rcc_name.to_string(), "RCC".to_string()),
                    family_clocks,
                );
            }
        }

        Ok(Self(peripheral_to_clock))
    }

    pub fn match_peri_clock(
        &self,
        rcc_block: &(String, String, String),
        peri_name: &str,
    ) -> Option<&stm32_data_serde::chip::core::peripheral::Rcc> {
        const PERI_OVERRIDE: &[(&str, &[&str])] = &[
            ("DCMI", &["DCMI_PSSI"]),
            ("PSSI", &["DCMI_PSSI"]),
            ("FDCAN1", &["FDCAN12", "FDCAN"]),
            ("FDCAN2", &["FDCAN12", "FDCAN"]),
            ("FDCAN3", &["FDCAN"]),
            ("ADC", &["ADC1"]),
            ("ADC1", &["ADC12", "ADC"]),
            ("ADC2", &["ADC12", "ADC"]),
            ("ADC3", &["ADC34", "ADC345", "ADC"]),
            ("ADC4", &["ADC34", "ADC345"]),
            ("ADC5", &["ADC345"]),
            ("DAC", &["DAC1"]),
            ("DAC1", &["DAC12", "DAC"]),
            ("DAC2", &["DAC12", "DAC"]),
            ("ETH", &["ETHMAC", "ETH1MAC"]),
        ];

        let clocks = self.0.get(rcc_block)?;
        if let Some(res) = clocks.get(peri_name) {
            return Some(res);
        }

        if let Some((_, rename)) = PERI_OVERRIDE.iter().find(|(n, _)| *n == peri_name) {
            for &n in *rename {
                if let Some(res) = clocks.get(n) {
                    return Some(res);
                }
            }
        }

        if let Some(n) = peri_name.strip_suffix('1') {
            if let Some(res) = clocks.get(n) {
                return Some(res);
            }
        }

        None
    }
}
