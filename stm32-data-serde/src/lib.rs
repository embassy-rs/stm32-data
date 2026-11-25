use serde::{Deserialize, Serialize};

#[macro_export]
macro_rules! regex {
    ($re:literal) => {{
        ::ref_thread_local::ref_thread_local! {
            static managed REGEX: ::regex::Regex = ::regex::Regex::new($re).unwrap();
        }
        <REGEX as ::ref_thread_local::RefThreadLocal<::regex::Regex>>::borrow(&REGEX)
    }};
}

fn is_default<T: Default + PartialEq>(variant: &T) -> bool {
    *variant == T::default()
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Chip {
    pub name: String,
    pub family: String,
    pub line: String,
    pub die: String,
    pub device_id: u16,
    pub packages: Vec<chip::Package>,
    pub memory: Vec<Vec<chip::Memory>>,
    pub docs: Vec<chip::Doc>,
    pub cores: Vec<chip::Core>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power: Option<chip::Power>,
}

pub mod chip {
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct Power {
        pub dialect: power::Dialect,
        #[serde(default, skip_serializing_if = "crate::is_false")]
        pub stopwuck: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub modes: Vec<power::Mode>,
    }

    pub mod power {
        use serde::{Deserialize, Serialize};

        /// PWR register layout / Stop entry-exit dialect for a chip family.
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub enum Dialect {
            /// `PWR.CR`: `PDDS` + `LPDS`/`LPSDSR` (F0, F1, F2, F3, F4, F7, L0, L1).
            Classic,
            /// `PWR.CR1.LPMS` + `PWR.SCR.CWUF` (G0, G4, L4, L5, U0).
            LpmsScr,
            /// `PWR.CR1.LPMS` + `PWR.SR.STOPF/CSSF` + `PWR.WUSCR` (U3, U5, WBA).
            LpmsSrWuscr,
            /// `PWR.PMCR/LPMCR.LPMS` + `SVOS` + `PWR.PMSR.STOPF` (C5, G5, H5).
            PmcrSvos,
            /// `PWR.EXTSCR.C1/C2STOPF/CSSF` + `CR1.LPMS` (WB, WL5, WLE).
            Extscr,
            /// Per-domain `CPUCR/CSR3` `PDDS` + `STOPF` (H7, H7RS, H7P, N6).
            DomainPdds,
            /// `PWR.CR1.LPMS` + `EXTSRR.DEEPSTOPF` (WB0).
            Wb0Deepstop,
        }

        #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub enum ModeKind {
            Stop0,
            Stop1,
            Stop2,
            Stop3,
        }

        #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct Mode {
            pub kind: ModeKind,
            /// Raw selector value written to `LPMS` (or `PDDS`/`LPDS` encoding for `Classic`).
            pub selector: u8,
            /// Minimum `SVOS` scale (3..=5) required for peripherals/wakeups in this mode.
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub min_svos: Option<u8>,
            /// Oscillator / clock names (matching `KernelClock` sources) that remain usable in this mode.
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            pub surviving_clocks: Vec<String>,
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct Package {
        pub name: String,
        pub package: String,
        pub pins: Vec<PackagePin>,
    }

    #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct PackagePin {
        pub position: String,
        pub signals: Vec<String>,
    }

    #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct Memory {
        pub name: String,
        pub kind: memory::Kind,
        pub address: u32,
        pub size: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub settings: Option<memory::Settings>,
        // probe-rs needs access attributes for automated tests
        #[serde(skip_serializing_if = "Option::is_none")]
        pub access: Option<memory::Access>,
    }

    pub mod memory {
        use serde::{Deserialize, Serialize};

        #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum Kind {
            Flash,
            Ram,
            Eeprom,
        }

        #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct Settings {
            pub erase_size: u32,
            pub write_size: u32,
            pub erase_value: u8,
        }

        #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct Access {
            pub read: bool,
            pub write: bool,
            pub execute: bool,
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct Doc {
        pub r#type: String,
        pub title: String,
        pub name: String,
        pub url: String,
    }

    #[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
    pub struct Core {
        pub name: String,
        pub peripherals: Vec<core::Peripheral>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub nvic_priority_bits: Option<u8>,
        pub interrupts: Vec<core::Interrupt>,
        pub dma_channels: Vec<core::DmaChannels>,
        pub pins: Vec<core::Pin>,
    }

    pub mod core {
        use serde::{Deserialize, Serialize};

        #[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
        pub struct Peripheral {
            pub name: String,
            #[serde(default)]
            pub address: u32,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub registers: Option<peripheral::Registers>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub rcc: Option<peripheral::Rcc>,
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            pub pins: Vec<peripheral::Pin>,
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            pub interrupts: Vec<peripheral::Interrupt>,
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            pub dma_channels: Vec<peripheral::DmaChannel>,
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            pub triggers: Vec<peripheral::Trigger>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub afio: Option<peripheral::Afio>,
        }

        pub mod peripheral {
            use serde::{Deserialize, Serialize};

            #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
            pub struct Registers {
                pub kind: String,
                pub version: String,
                pub block: String,
            }

            #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
            pub struct Rcc {
                pub bus_clock: String,
                pub kernel_clock: rcc::KernelClock,
                pub enable: rcc::Field,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub reset: Option<rcc::Field>,
                #[serde(default, skip_serializing_if = "Option::is_none")]
                pub stop_enable: Option<rcc::Field>,
                #[serde(default, skip_serializing_if = "Option::is_none")]
                pub autonomous_enable: Option<rcc::Field>,
                #[serde(default, skip_serializing_if = "crate::is_default")]
                pub low_power: rcc::LowPower,
            }

            pub mod rcc {
                use serde::{Deserialize, Serialize};

                #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
                pub struct Field {
                    pub register: String,
                    pub field: String,
                }

                #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
                #[serde(untagged)]
                pub enum KernelClock {
                    Clock(String),
                    Mux(Mux),
                }

                /// A peripheral kernel clock.
                ///
                /// One struct covers every historical shape: a plain mux
                /// (`{"register","field"}`), a mux with a divider applied to its output
                /// (`{"register","field","prescaler"}`, e.g. C5 `CCIPR2/ADCDACSEL` plus
                /// `ADCDACPRE`), and - when there is no mux - a parent clock behind a
                /// divider (`{"clock","prescaler"}`, e.g. F1 `PCLK2` behind
                /// `CFGR/ADCPRE`).
                ///
                /// Invariant: exactly one of `clock` / (`register` + `field`) is set;
                /// `prescaler` is optional on top. Every field being optional is safe for
                /// serde's untagged deserialization because there is only one struct
                /// variant - the fields present determine the meaning, so the old
                /// "PrescaledMux must come before Mux" ordering hazard cannot occur - and
                /// `skip_serializing_if` keeps the output byte-identical to the
                /// historical shapes.
                #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
                pub struct Mux {
                    /// Parent clock when the peripheral has no mux field.
                    #[serde(default, skip_serializing_if = "Option::is_none")]
                    pub clock: Option<String>,
                    #[serde(default, skip_serializing_if = "Option::is_none")]
                    pub register: Option<String>,
                    #[serde(default, skip_serializing_if = "Option::is_none")]
                    pub field: Option<String>,
                    /// Divider applied to `clock` or to the mux output (e.g. the C5
                    /// `CCIPR2/ADCDACPRE` divider after `CCIPR2/ADCDACSEL`).
                    #[serde(default, skip_serializing_if = "Option::is_none")]
                    pub prescaler: Option<Field>,
                }

                impl Mux {
                    /// Reject combinations serde's untagged, all-optional shape cannot:
                    /// a typo'd key deserializes to an all-`None` mux, and a half-set
                    /// `register`/`field` pair is meaningless.
                    pub fn validate(&self) -> Result<(), String> {
                        let has_parent = self.clock.is_some();
                        let has_mux = self.register.is_some() || self.field.is_some();
                        if self.register.is_some() != self.field.is_some() {
                            return Err("mux register and field must be set together".into());
                        }
                        if has_parent == has_mux {
                            return Err(if has_parent {
                                "clock and mux are mutually exclusive".into()
                            } else {
                                "neither clock nor mux set".into()
                            });
                        }
                        Ok(())
                    }
                }

                /// Per-peripheral Stop-mode capabilities and wake-up routing.
                #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
                pub struct LowPower {
                    /// Stop modes in which this peripheral remains functional (assuming its clock survives).
                    #[serde(default, skip_serializing_if = "crate::is_default")]
                    pub modes: StopModes,
                    /// Can autonomously request its clock / operate in Stop without CPU intervention.
                    #[serde(default, skip_serializing_if = "crate::is_false")]
                    pub autonomous: bool,
                    /// Can raise a wake-up IRQ/event from a Stop mode listed in `modes`.
                    #[serde(default, skip_serializing_if = "crate::is_false")]
                    pub wake_capable: bool,
                    /// EXTI line associated with this peripheral's Stop wake-up when routed via EXTI.
                    #[serde(default, skip_serializing_if = "Option::is_none")]
                    pub wake_exti: Option<u8>,
                }

                #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
                pub struct StopModes {
                    #[serde(default, skip_serializing_if = "crate::is_false")]
                    pub stop0: bool,
                    #[serde(default, skip_serializing_if = "crate::is_false")]
                    pub stop1: bool,
                    #[serde(default, skip_serializing_if = "crate::is_false")]
                    pub stop2: bool,
                    #[serde(default, skip_serializing_if = "crate::is_false")]
                    pub stop3: bool,
                }

                impl StopModes {
                    pub const fn any(&self) -> bool {
                        self.stop0 || self.stop1 || self.stop2 || self.stop3
                    }

                    pub const fn contains(&self, kind: crate::chip::power::ModeKind) -> bool {
                        match kind {
                            crate::chip::power::ModeKind::Stop0 => self.stop0,
                            crate::chip::power::ModeKind::Stop1 => self.stop1,
                            crate::chip::power::ModeKind::Stop2 => self.stop2,
                            crate::chip::power::ModeKind::Stop3 => self.stop3,
                        }
                    }
                }
            }

            #[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
            pub struct Pin {
                pub pin: String,
                pub signal: String,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub af: Option<u8>,
            }

            #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
            pub struct Interrupt {
                pub signal: String,
                pub interrupt: String,
            }

            #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
            pub struct Trigger {
                pub signal: String,
                pub source: String,
            }

            #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
            pub struct DmaChannel {
                pub signal: String,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub dma: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub channel: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub dmamux: Option<String>,
                #[serde(default, skip_serializing_if = "Vec::is_empty")]
                pub remap: Vec<RemapInfo>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub request: Option<u8>,
            }

            #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
            pub struct RemapInfo {
                pub register: String,
                pub field: String,
                pub value: u8,
            }

            #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
            pub struct Afio {
                pub register: String,
                pub field: String,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub values: Vec<AfioValue>,
            }

            #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
            pub struct AfioValue {
                pub value: u8,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub pins: Vec<String>,
            }
        }

        #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct Interrupt {
            pub name: String,
            pub number: u8,
        }

        #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct Pin {
            pub name: String,
        }

        #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct DmaChannels {
            pub name: String,
            pub dma: String,
            pub channel: u8,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub dmamux: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub dmamux_channel: Option<u8>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub supports_2d: Option<bool>,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::path::Path;
    use std::{fs, str};

    use super::*;

    fn normalize_line(line: &str) -> Cow<'_, str> {
        // The python script saves with 4 spaces instead of 2
        let line = line.trim_start();

        // The python script escapes unicode
        let mut line = Cow::Borrowed(line);
        for symbol in [("\\u00ae", "\u{00ae}"), ("\\u2122", "\u{2122}")] {
            if line.contains(symbol.0) {
                line = Cow::Owned(line.replace(symbol.0, symbol.1));
            }
        }

        line
    }

    fn normalize(file: &[u8]) -> impl Iterator<Item = Cow<'_, str>> + '_ {
        str::from_utf8(file).unwrap().lines().map(normalize_line)
    }

    fn check_file(path: impl AsRef<Path>) {
        println!("Checking {:?}", path.as_ref());
        let original = fs::read(path).unwrap();
        let parsed: Chip = serde_json::from_slice(&original).unwrap();
        let reencoded = serde_json::to_vec_pretty(&parsed).unwrap();
        itertools::assert_equal(normalize(&original), normalize(&reencoded))
    }

    const CHIPS_DIR: &str = "../build/data/chips/";

    #[test]
    fn test_one() {
        let path = Path::new(CHIPS_DIR).join("STM32F030C6.json");
        check_file(path);
    }

    #[test]
    fn test_all() {
        use rayon::prelude::*;

        Path::new(CHIPS_DIR).read_dir().unwrap().par_bridge().for_each(|chip| {
            check_file(chip.unwrap().path());
        });
    }

    /// Pin down the individual `KernelClock` shapes: historical JSON must round-trip
    /// byte-identically, and invalid combinations must be rejected by `validate` (serde
    /// itself accepts them, since the `Mux` struct is all-optional).
    #[test]
    fn kernel_clock_shapes() {
        use crate::chip::core::peripheral::rcc::KernelClock;

        let shapes: &[(&str, bool)] = &[
            // plain mux
            (r#"{"register":"CCIPR1","field":"I2C1SEL"}"#, true),
            // mux + prescaler (C5 ADC/DAC)
            (
                r#"{"register":"CCIPR2","field":"ADCDACSEL","prescaler":{"register":"CCIPR2","field":"ADCDACPRE"}}"#,
                true,
            ),
            // prescaler over a parent clock (F1 ADC)
            (
                r#"{"clock":"PCLK2","prescaler":{"register":"CFGR","field":"ADCPRE"}}"#,
                true,
            ),
            // invalid: neither clock nor mux
            (r#"{}"#, false),
            // invalid: half a mux
            (r#"{"register":"CFGR"}"#, false),
            // invalid: clock and mux at once
            (r#"{"clock":"PCLK2","register":"CFGR","field":"SW"}"#, false),
        ];

        for (json, valid) in shapes {
            let parsed: KernelClock = serde_json::from_str(json).unwrap();
            match &parsed {
                KernelClock::Clock(_) => panic!("unexpected clock string in {}", json),
                KernelClock::Mux(mux) => assert_eq!(mux.validate().is_ok(), *valid, "{}", json),
            }
            if *valid {
                // byte-identical round-trip (field order = declaration order)
                assert_eq!(serde_json::to_string(&parsed).unwrap(), *json, "{}", json);
            }
        }

        // plain clock string
        let parsed: KernelClock = serde_json::from_str(r#""PCLK2""#).unwrap();
        assert!(matches!(parsed, KernelClock::Clock(_)));
        assert_eq!(serde_json::to_string(&parsed).unwrap(), r#""PCLK2""#);
    }
}
