use std::collections::{HashMap, HashSet};

use anyhow::{Ok, anyhow, bail};
use chiptool::ir::IR;
use lazy_regex::regex;
use stm32_data_serde::chip::core::peripheral::rcc::{Field, LowPower};
use stm32_data_serde::chip::core::peripheral::{self, rcc};

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
    /// name -> sleep/stop clock enable bit (*SMENR, *LPENR, *SLPENR)
    stop_enable: HashMap<String, Field>,
    /// name -> autonomous stop clock enable bit (SRDAMR/*AMEN, *STPENR)
    autonomous_enable: HashMap<String, Field>,
    /// name -> mux info
    mux: HashMap<String, MuxInfo>,
    /// name -> prescaler info for peripherals clocked through a bare divider field
    /// (no mux in front of it, e.g. F1 `CFGR/ADCPRE`)
    prescaler: HashMap<String, Field>,
    /// Whether this RCC block has a `STOPWUCK` wake-up clock selector field.
    stopwuck: bool,
}

#[derive(Debug)]
struct MuxInfo {
    mux: Field,
    variants: Vec<String>,
    /// Divider applied to the mux output, in the same register as the mux (if any).
    prescaler: Option<Field>,
}

#[derive(Debug, Clone)]
struct EnRst {
    enable: rcc::Field,
    reset: Option<rcc::Field>,
    bus_clock: String,
}

impl ParsedRccs {
    /// Parse the RCC information from the `rcc_xx` yaml files in `data/registers`
    pub fn parse(registers: &Registers) -> anyhow::Result<Self> {
        let mut rccs = HashMap::new();

        for (rcc_name, ir) in &registers.registers {
            if let Some(rcc_name) = rcc_name.strip_prefix("rcc_") {
                rccs.insert(rcc_name.to_string(), Self::parse_rcc(rcc_name, ir)?);
            }
        }

        Ok(Self { rccs })
    }

    /// Parse mcu specific RCC information from the IR object
    /// - Clock source muxes of peripherals
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
            "HCLK",
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
            "PLL1_S",
            "PLL1_T",
            "PLL1_VCO", // used for L0 USB
            "PLL2_P",
            "PLL2_Q",
            "PLL2_R",
            "PLL2_S",
            "PLL2_T",
            "PLL3_P",
            "PLL3_Q",
            "PLL3_R",
            "PLL3_S",
            "PLL3_T",
            "HSI",
            "HSIDIV3",
            "HSIK",
            "PSI",
            "PSIDIV3",
            "PSIK",
            "SHSI",
            "HSI48",
            "HSIKER",
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
            "DSI_PHY",
            // TODO: variants to cleanup
            "AFIF",
            "HSI_HSE",
            "SAI1_EXTCLK",
            "SAI2_EXTCLK",
            "I2S_CKIN",
            "DAC_HOLD",
            "DAC_HOLD_2",
            "ICLK",
            // G5: DACSEL can select the ADC kernel clock (rcc_adc_ker_ck).
            "ADC",
            "DCLK",
            "I2S1",
            "I2S2",
            "SAI1",
            "SAI2",
            "SPDIFRX_SYMB",
            "ETH_RMII_REF",
            "ETH_CLK_FB",
            "ETH",
            "CLK48MOHCI",
            "DIV_RTCPRE",
            "HSE_DIV_RTCPRE",
            // N6 extra
            "IC1",
            "IC2",
            "IC3",
            "IC4",
            "IC5",
            "IC6",
            "IC7",
            "IC8",
            "IC9",
            "IC10",
            "IC11",
            "IC12",
            "IC13",
            "IC14",
            "IC15",
            "IC16",
            "IC17",
            "IC18",
            "IC19",
            "IC20",
            "HSI_DIV",
            "HSE_RTC",
            "HSE_DIV2_OSC", // TODO: remove this
            "TIMG",
            // H7P: rcc_timx_ker_ck, ADC/DAC kernel clock source
            "TIMX_KER",
            "HCLKE", // ethernet clock
            "MII",
            "RGMII",
            "JTAG_TCK",
            "SYSA",
            "SYSB",
            "SPDIF_SYMB",
            "HCLKU",
        ]);

        let mux_regexes = &[
            regex!(r"^DCKCFGR\d?/(.+)SEL$"),
            regex!(r"^CCIPR\d*/(.+)SEL$"),
            regex!(r"^BDCR\d?/(.+)SEL$"),
            regex!(r"^D\dCCIP\d?R/(.+)SEL$"),
            regex!(r"^CFGR\d/(.+)SW$"),
            regex!(r"^.+PERCKSELR/(.+)SEL$"),
            regex!(r"^CSR/(.+)SEL$"),
        ];
        let mux_nopelist = &[
            regex!(r"^.+PERCKSELR/USBREFCKSEL$"),
            regex!(r"^CCIPR\d*/OTGPHYREFCKSEL$"),
            regex!(r"^.+/TIMICSEL$"),
        ];

        let mut mux = HashMap::new();
        for (reg, body) in &ir.fieldsets {
            for field in &body.fields {
                let key = format!("{}/{}", reg, field.name);
                if let Some(capture) = mux_regexes.iter().find_map(|r| r.captures(&key)) {
                    if mux_nopelist.iter().any(|r| r.is_match(&key)) {
                        continue;
                    }

                    let peri = capture.get(1).unwrap().as_str();

                    // TODO: these bits are duplicated on F4, we need to split the F4 RCCs more.
                    if rcc_version.starts_with("f4") && reg == "DCKCFGR2" && (peri == "CLK48" || peri == "SDIO") {
                        continue;
                    }

                    // ignore switches with missing enum.
                    let Some(enum_name) = field.enumm.as_deref() else {
                        continue;
                    };

                    let enumm = ir.enums.get(enum_name).unwrap();
                    for v in &enumm.variants {
                        let mut vname = v.name.as_str();
                        if let Some(captures) = regex!(r"^([A-Z0-9_]+)_DIV_\d+?$").captures(v.name.as_str()) {
                            vname = captures.get(1).unwrap().as_str();
                        }
                        if let Some(captures) = regex!(r"^([A-Z0-9_]+)_MUL_\d+?$").captures(v.name.as_str()) {
                            vname = captures.get(1).unwrap().as_str();
                        }

                        if !allowed_variants.contains(vname) {
                            return Err(anyhow!(
                                "rcc: prohibited variant name {} in enum {} for rcc_{}",
                                v.name.as_str(),
                                enum_name,
                                rcc_version
                            ));
                        }
                    }

                    // Some muxes are followed by a prescaler in the same register, which the
                    // clock tree model has to account for (C5: CCIPR2/ADCDACSEL is followed by
                    // CCIPR2/ADCDACPRE). The mux regexes above only match `*SEL` fields, so look
                    // for the divider explicitly. Only accept fields whose enum is a divider so
                    // that unrelated `*PRE` fields are not picked up.
                    let prescaler =
                        body.fields
                            .iter()
                            .find(|f| {
                                (f.name == format!("{peri}PRE") || f.name == format!("{peri}PRESC"))
                                    && f.enumm.as_deref().and_then(|e| ir.enums.get(e)).is_some_and(|e| {
                                        e.variants.iter().all(|v| regex!(r"^(Div|Mul)").is_match(&v.name))
                                    })
                            })
                            .map(|f| Field {
                                register: reg.clone(),
                                field: f.name.clone(),
                            });

                    let val = MuxInfo {
                        mux: Field {
                            register: reg.clone(),
                            field: field.name.clone(),
                        },
                        variants: enumm.variants.iter().map(|v| v.name.clone()).collect(),
                        prescaler,
                    };

                    if mux.insert(peri.to_string(), val).is_some() {
                        bail!("rcc: duplicate mux for {} for rcc_{}", peri, rcc_version);
                    }
                }
            }
        }

        // Some peripherals are clocked through a bare divider field with no mux in
        // front of it (F1: `CFGR/ADCPRE` divides `PCLK2` for the ADCs). Model those as
        // a prescaler over the bus clock. Only combinations that embassy-stm32
        // actually programs are listed: `f013.rs` writes `ADCPRE` only under
        // `#[cfg(stm32f1)]`, while on F0/F3/F37 the same-named field does not gate the
        // kernel clock those ports use (ADC CKMODE/ADC12PRES), so modeling it there
        // would report a wrong frequency.
        const PRESCALER_ONLY: &[(&str, &str)] = &[("f1", "ADC"), ("f1cl", "ADC"), ("f100", "ADC")];

        let mut prescaler = HashMap::new();
        if PRESCALER_ONLY.iter().any(|(v, _)| *v == rcc_version) {
            for (reg, body) in &ir.fieldsets {
                for field in &body.fields {
                    let Some(peri) = field
                        .name
                        .strip_suffix("PRESC")
                        .or_else(|| field.name.strip_suffix("PRE"))
                        .filter(|peri| !peri.is_empty())
                    else {
                        continue;
                    };
                    if !PRESCALER_ONLY.contains(&(rcc_version, peri)) {
                        continue;
                    }
                    // Only accept divider enums so unrelated `*PRE` fields are not
                    // picked up.
                    if !field
                        .enumm
                        .as_deref()
                        .and_then(|e| ir.enums.get(e))
                        .is_some_and(|e| e.variants.iter().all(|v| regex!(r"^(Div|Mul)").is_match(&v.name)))
                    {
                        continue;
                    }
                    if prescaler
                        .insert(
                            peri.to_string(),
                            Field {
                                register: reg.clone(),
                                field: field.name.clone(),
                            },
                        )
                        .is_some()
                    {
                        bail!("rcc: duplicate prescaler {} for rcc_{}", peri, rcc_version);
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
                    if let Some(bit_name) = field.name.strip_suffix("EN") {
                        let peri = match bit_name {
                            "RTCAPB" => "RTC",
                            // C0, G0 and L0 call the DBGMCU clock DBGEN/DBGRST.
                            "DBG" => "DBGMCU",
                            peri => peri,
                        };
                        let reset_name = if bit_name == "DBG" { bit_name } else { peri };

                        let mut reset = None;
                        if let Some(rstr) = ir.fieldsets.get(&reg.replace("ENR", "RSTR"))
                            && let Some(_field) = rstr
                                .fields
                                .iter()
                                .find(|field| field.name == format!("{reset_name}RST"))
                        {
                            reset = Some(rcc::Field {
                                register: reg.replace("ENR", "RSTR"),
                                field: format!("{reset_name}RST"),
                            });
                        }

                        let clock = clock.replace("AHB", "HCLK").replace("APB", "PCLK");

                        let val = EnRst {
                            enable: rcc::Field {
                                register: reg.clone(),
                                field: field.name.clone(),
                            },
                            reset,
                            bus_clock: clock,
                        };

                        if en_rst.insert(peri.to_string(), val).is_some() {
                            bail!("rcc: duplicate en/rst for {} for rcc_{}", peri, rcc_version);
                        }
                    }
                }
            }
        }

        fn normalize_lp_peri(bit_name: &str) -> &str {
            match bit_name {
                "RTCAPB" => "RTC",
                "DBG" => "DBGMCU",
                "SP3" => "SPI3",
                "SPDIF" => "SPDIFRX",
                "CRSS" => "CRS",
                "ADCFS" => "ADC",
                "HASH1" => "HASH",
                other => other,
            }
        }

        // Parse xxSMEN / xxLPEN / xxSLPEN bits.
        let mut stop_enable = HashMap::new();
        for (reg, body) in &ir.fieldsets {
            let Some(m) = regex!(r"^((?:A[PH]B\d?|GPIO)[LH]?(SMEN|SLPEN|LPEN)R\d?|GPIOSMEN)$").captures(reg) else {
                continue;
            };
            let suffix = m.get(2).map_or("SMEN", |g| g.as_str());
            for field in &body.fields {
                let bit_name = if suffix == "SMEN" {
                    field
                        .name
                        .strip_suffix("SMEN")
                        .or_else(|| field.name.strip_suffix("MEN"))
                        .or_else(|| (rcc_version == "g4" && field.name == "RNGEN").then_some("RNG"))
                } else {
                    field.name.strip_suffix(suffix)
                };
                let Some(bit_name) = bit_name else {
                    continue;
                };
                let peri = normalize_lp_peri(bit_name);
                stop_enable.insert(
                    peri.to_string(),
                    Field {
                        register: reg.clone(),
                        field: field.name.clone(),
                    },
                );
            }
        }

        // Parse autonomous Stop-mode clock enable bits (`SRDAMR/*AMEN`, `*STPENR/*STPEN`).
        let mut autonomous_enable = HashMap::new();
        for (reg, body) in &ir.fieldsets {
            let suffix = if reg == "SRDAMR" {
                "AMEN"
            } else if regex!(r"^(?:A[PH]B\d?|GPIO)[LH]?STPENR\d?$").is_match(reg) {
                "STPEN"
            } else {
                continue;
            };
            for field in &body.fields {
                let Some(bit_name) = field.name.strip_suffix(suffix) else {
                    continue;
                };
                let peri = normalize_lp_peri(bit_name);
                autonomous_enable.insert(
                    peri.to_string(),
                    Field {
                        register: reg.clone(),
                        field: field.name.clone(),
                    },
                );
            }
        }

        let stopwuck = ir
            .fieldsets
            .values()
            .any(|body| body.fields.iter().any(|f| f.name == "STOPWUCK"));

        Ok(ParsedRcc {
            en_rst,
            stop_enable,
            autonomous_enable,
            mux,
            prescaler,
            stopwuck,
        })
    }

    pub fn has_stopwuck(&self, rcc_version: &str) -> bool {
        self.rccs.get(rcc_version).is_some_and(|rcc| rcc.stopwuck)
    }

    pub fn match_peri_clock(
        &self,
        rcc_version: &str,
        peri_name: &str,
    ) -> Option<stm32_data_serde::chip::core::peripheral::Rcc> {
        const FALLBACKS: &[(&str, &[&str])] = &[
            ("DCMI", &["DCMI_PSSI", "PSSI"]),
            ("PSSI", &["DCMI_PSSI", "DCMI"]),
            ("FMC", &["FSMC"]),
            ("FDCAN1", &["FDCAN12", "FDCAN"]),
            ("FDCAN2", &["FDCAN12", "FDCAN1", "FDCAN"]),
            ("FDCAN3", &["FDCAN"]),
            ("ADC", &["ADC1", "ADCDAC"]),
            ("ADC1", &["ADC12", "ADCDAC"]),
            ("ADC2", &["ADC12", "ADCDAC"]),
            ("ADC3", &["ADC34", "ADC345", "ADCDAC"]),
            ("ADC4", &["ADC34", "ADC345", "ADCDAC"]),
            ("ADC5", &["ADC345", "ADCDAC"]),
            ("DAC", &["DAC1", "ADCDAC"]),
            ("DAC1", &["DAC12", "ADCDAC"]),
            ("DAC2", &["DAC12", "ADCDAC"]),
            ("DSIHOST", &["DSI"]),
            ("ETH", &["ETHMAC", "ETH1MAC"]),
            ("ETH1", &["ETH1CLK"]),
            ("SPI1", &["SPI12", "SPI123"]),
            ("SPI2", &["SPI12", "SPI123", "SPI23"]),
            ("SPI3", &["SPI123", "SPI23"]),
            ("SPI4", &["SPI145", "SPI45"]),
            ("SPI5", &["SPI145", "SPI45"]),
            ("SAI1", &["SAI12"]),
            ("SAI2", &["SAI12", "SAI23"]),
            ("SAI3", &["SAI23"]),
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
            ("LPTIM2", &["LPTIM23"]),
            ("LPTIM3", &["LPTIM23", "LPTIM34", "LPTIM345"]),
            ("LPTIM4", &["LPTIM34", "LPTIM345", "LPTIM45"]),
            ("LPTIM5", &["LPTIM345", "LPTIM45"]),
            ("I2C1", &["I2C1235", "I2C1_I3C1"]),
            ("I3C1", &["I2C1_I3C1"]),
            ("I2C2", &["I2C1235"]),
            ("I2C3", &["I2C1235"]),
            ("I2C5", &["I2C1235"]),
            ("USB", &["USB", "CLK48", "ICLK"]),
            ("USB_OTG_FS", &["USB", "CLK48", "ICLK", "OTGFS"]),
            ("USB_OTG_HS", &["USB", "USBPHYC", "OTGHS", "CLK48", "ICLK"]),
            ("USB1_OTG_HS", &["OTG1", "OTGPHY1"]),
            ("USB2_OTG_HS", &["OTG2", "OTGPHY2"]),
            ("OTG1PHYCTL", &["OTGPHY1"]),
            ("OTG2PHYCTL", &["OTGPHY2"]),
            ("DTS", &["TMPSENS"]),
            ("SDMMC1", &["SDMMC1", "CLK48"]),
            ("SYSCFG", &["SBS"]),
            ("RNG", &["CLK48"]),
            ("IPCC", &["RFWKP"]),
        ];

        let rcc = self.rccs.get(rcc_version)?;

        let en_rst = get_with_fallback(peri_name, &rcc.en_rst, FALLBACKS)?;
        let stop_enable = get_with_fallback(peri_name, &rcc.stop_enable, FALLBACKS).cloned();
        let autonomous_enable = get_with_fallback(peri_name, &rcc.autonomous_enable, FALLBACKS).cloned();
        let mux = get_with_fallback(peri_name, &rcc.mux, FALLBACKS);
        let prescaler = get_with_fallback(peri_name, &rcc.prescaler, FALLBACKS);

        let phclk = regex!("^[PH]CLK");

        let mut maybe_kernel_clock = en_rst.bus_clock.clone();
        // Timers are a bit special, they may have a x2 freq
        if regex!(r"^(HR)?TIM\d+$").is_match(peri_name) {
            maybe_kernel_clock.push_str("_TIM");
        }

        const RCC_PERI_MUX_EXCEPTIONS: &[(&str, &str)] = &[
            // These peripherals have a different mux name than the bus clock
            // Format: rcc_version, peripheral_name
            ("u3", "ADC"),
            ("u3", "DAC"),
            ("u5", "ADC"),
            ("n6", "I2C4"),
            ("n6", "SDMMC1"), // HCLK2 is corrext per Cube and Docs so no mux check
            ("n6", "SDMMC2"), // HCLKU is corrext per Cube and Docs so no mux check
            ("n6", "ETH1"),   // HCLKE is corrext per RM0486 Figure "Core and bus clock generation" so no mux check
        ];

        let kernel_clock = match mux {
            Some(mux) => {
                // check for mismatch between mux and bus clock.
                //
                // U5 has one ADCDACSEL for multiple ADCs which may be on
                // different HCLKs, so we skip the check in that case
                if !(RCC_PERI_MUX_EXCEPTIONS
                    .iter()
                    .any(|x| rcc_version == x.0 && peri_name.starts_with(x.1)))
                    && phclk.is_match(&en_rst.bus_clock)
                    && rcc_version != "c5"
                {
                    for v in &mux.variants {
                        if phclk.is_match(v) && v != &maybe_kernel_clock {
                            panic!(
                                "rcc_{}: peripheral {} is on bus {} but mux {}.{} refers to {}",
                                rcc_version, peri_name, maybe_kernel_clock, mux.mux.register, mux.mux.field, v
                            )
                        }
                    }
                }
                rcc::KernelClock::Mux(rcc::Mux {
                    clock: None,
                    register: Some(mux.mux.register.clone()),
                    field: Some(mux.mux.field.clone()),
                    prescaler: mux.prescaler.clone(),
                })
            }
            None => {
                if peri_name.starts_with("USB") {
                    if rcc_version.starts_with("f1") || rcc_version.starts_with("f3") {
                        maybe_kernel_clock = "USB".to_string();
                    } else if rcc_version.starts_with("f2") {
                        maybe_kernel_clock = "PLL1_Q".to_string();
                    } else if rcc_version.starts_with("l1") {
                        maybe_kernel_clock = "PLL1_VCO_DIV2".to_string();
                    } else if rcc_version.starts_with("h7rs") {
                        maybe_kernel_clock = "USB".to_string();
                    } else {
                        panic!("rcc_{}: peripheral {} missing mux", rcc_version, peri_name)
                    }
                }
                if rcc_version.starts_with("h7") && peri_name.starts_with("LTDC") {
                    maybe_kernel_clock = "PLL3_R".to_string();
                }
                match prescaler {
                    Some(prescaler) => rcc::KernelClock::Mux(rcc::Mux {
                        clock: Some(maybe_kernel_clock),
                        register: None,
                        field: None,
                        prescaler: Some(prescaler.clone()),
                    }),
                    None => rcc::KernelClock::Clock(maybe_kernel_clock),
                }
            }
        };

        Some(peripheral::Rcc {
            bus_clock: en_rst.bus_clock.clone(),
            kernel_clock,
            enable: en_rst.enable.clone(),
            reset: en_rst.reset.clone(),
            stop_enable,
            autonomous_enable,
            low_power: LowPower::default(),
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

    if let Some(capture) = regex!("^([A-Z]+)\\d+$").captures(key)
        && let Some(res) = map.get(capture.get(1).unwrap().as_str())
    {
        return Some(res);
    }

    None
}
