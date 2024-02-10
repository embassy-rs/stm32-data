use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, bail, Ok};
use chiptool::ir::{BlockItemInner, Enum, IR};
use stm32_data_serde::chip::core::peripheral::rcc::{Mux, StopMode};
use stm32_data_serde::chip::core::peripheral::{self, rcc};

use crate::regex;
use crate::registers::Registers;

#[derive(Debug)]
pub struct ParsedRccs {
    /// RCC version -> parsed info
    rccs: HashMap<String, ParsedRcc>,
}

#[derive(Debug)]
struct ParsedRcc {
    /// name -> en/rst bit info
    en_rst: HashMap<String, EnRst>,
    /// name -> mux info
    mux: HashMap<String, Mux>,
}

#[derive(Debug)]
struct EnRst {
    enable: rcc::Enable,
    reset: Option<rcc::Reset>,
    clock: String,
    stop_mode: StopMode,
}

impl ParsedRccs {
    pub fn parse(registers: &Registers) -> anyhow::Result<Self> {
        let mut rccs = HashMap::new();

        for (rcc_name, ir) in &registers.registers {
            if let Some(rcc_name) = rcc_name.strip_prefix("rcc_") {
                rccs.insert(rcc_name.to_string(), Self::parse_rcc(rcc_name, ir)?);
            }
        }

        Ok(Self { rccs })
    }

    fn parse_rcc(rcc_version: &str, ir: &IR) -> anyhow::Result<ParsedRcc> {
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
                            rcc_version
                        ));
                    }
                } else if !allowed_variants.contains(v.name.as_str()) {
                    return Err(anyhow!(
                        "rcc: prohibited variant name {} in enum {} for rcc_{}",
                        v.name.as_str(),
                        enumm_name,
                        rcc_version
                    ));
                }
            }

            Ok(())
        };

        let mux_regexes = &[
            regex!(r"^DCKCFGR\d?/(.+)SEL$"),
            regex!(r"^CCIPR\d?/(.+)SEL$"),
            regex!(r"^D\dCCIP\d?R/(.+)SEL$"),
            regex!(r"^CFGR\d/(.+)SW$"),
        ];

        let mut mux = HashMap::new();
        for (reg, body) in &ir.fieldsets {
            for field in &body.fields {
                let key = format!("{}/{}", reg, field.name);
                if let Some(capture) = mux_regexes.iter().find_map(|r| r.captures(&key)) {
                    let peri = capture.get(1).unwrap().as_str();

                    // TODO: these bits are duplicated on F4, we need to split the F4 RCCs more.
                    if rcc_version.starts_with("f4") && reg == "DCKCFGR2" && (peri == "CLK48" || peri == "SDIO") {
                        continue;
                    }

                    check_mux(reg, &field.name)?;

                    let val = Mux {
                        register: reg.to_ascii_lowercase(),
                        field: field.name.to_ascii_lowercase(),
                    };

                    if mux.insert(peri.to_string(), val).is_some() {
                        bail!("rcc: duplicate mux for {} for rcc_{}", peri, rcc_version);
                    }
                }
            }
        }

        // Parse xxEN/xxRST bits.
        let mut en_rst = HashMap::new();
        for (reg, body) in &ir.fieldsets {
            if let Some(m) = regex!(r"^((A[PH]B\d?)|GPIO)[LH]?ENR\d?$").captures(reg) {
                let clock = m.get(1).unwrap().as_str();
                let clock = match clock {
                    "AHB" => "AHB1",
                    "APB" => "APB1",
                    clock => clock,
                };

                for field in &body.fields {
                    if let Some(peri) = field.name.strip_suffix("EN") {
                        let peri = if peri == "RTCAPB" { "RTC" } else { peri };

                        let mut reset = None;
                        if let Some(rstr) = ir.fieldsets.get(&reg.replace("ENR", "RSTR")) {
                            if let Some(_field) = rstr.fields.iter().find(|field| field.name == format!("{peri}RST")) {
                                reset = Some(stm32_data_serde::chip::core::peripheral::rcc::Reset {
                                    register: reg.replace("ENR", "RSTR").to_ascii_lowercase(),
                                    field: format!("{peri}RST").to_ascii_lowercase(),
                                });
                            }
                        }

                        let stop_mode = if peri == "RTC" {
                            StopMode::Standby
                        } else if peri.starts_with("LP") {
                            StopMode::Stop2
                        } else {
                            StopMode::Stop1
                        };

                        // Timers are a bit special, they may have a x2 freq
                        let peri_clock = if regex!(r"^TIM\d+$").is_match(peri) {
                            format!("{clock}_TIM")
                        } else {
                            clock.to_string()
                        };

                        let peri_clock = peri_clock
                            .to_ascii_lowercase()
                            .replace("ahb", "hclk")
                            .replace("apb", "pclk");

                        let val = EnRst {
                            enable: peripheral::rcc::Enable {
                                register: reg.to_ascii_lowercase(),
                                field: field.name.to_ascii_lowercase(),
                            },
                            reset,
                            clock: peri_clock,
                            stop_mode,
                        };

                        if en_rst.insert(peri.to_string(), val).is_some() {
                            bail!("rcc: duplicate en/rst for {} for rcc_{}", peri, rcc_version);
                        }
                    }
                }
            }
        }

        Ok(ParsedRcc { en_rst, mux })
    }

    pub fn match_peri_clock(
        &self,
        rcc_version: &str,
        peri_name: &str,
    ) -> Option<stm32_data_serde::chip::core::peripheral::Rcc> {
        const FALLBACKS: &[(&str, &[&str])] = &[
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
            ("SPI1", &["SPI12", "SPI123"]),
            ("SPI2", &["SPI12", "SPI123"]),
            ("SPI3", &["SPI123"]),
            ("SPI4", &["SPI145"]),
            ("SPI5", &["SPI145"]),
            ("SAI1", &["SAI12"]),
            ("SAI2", &["SAI12"]),
            ("USART2", &["USART234578"]),
            ("USART3", &["USART234578"]),
            ("UART4", &["USART234578"]),
            ("UART5", &["USART234578"]),
            ("UART7", &["USART234578"]),
            ("UART8", &["USART234578"]),
            ("USART1", &["USART16910"]),
            ("USART6", &["USART16910"]),
            ("USART10", &["USART16910"]),
            ("UART9", &["USART16910"]),
            ("I2C1", &["I2C1235"]),
            ("I2C2", &["I2C1235"]),
            ("I2C3", &["I2C1235"]),
            ("I2C5", &["I2C1235"]),
        ];

        let rcc = self.rccs.get(rcc_version)?;

        let en_rst = get_with_fallback(peri_name, &rcc.en_rst, FALLBACKS)?;
        let mux = get_with_fallback(peri_name, &rcc.mux, FALLBACKS);

        Some(peripheral::Rcc {
            clock: en_rst.clock.clone(),
            enable: en_rst.enable.clone(),
            reset: en_rst.reset.clone(),
            stop_mode: en_rst.stop_mode.clone(),

            mux: mux.cloned(),
        })
    }
}

fn get_with_fallback<'a, T>(key: &str, map: &'a HashMap<String, T>, fallbacks: &[(&str, &[&str])]) -> Option<&'a T> {
    if let Some(res) = map.get(key) {
        return Some(res);
    }

    if let Some((_, rename)) = fallbacks.iter().find(|(n, _)| *n == key) {
        for &n in *rename {
            if let Some(res) = map.get(n) {
                return Some(res);
            }
        }
    }

    if let Some(n) = key.strip_suffix('1') {
        if let Some(res) = map.get(n) {
            return Some(res);
        }
    }

    None
}
