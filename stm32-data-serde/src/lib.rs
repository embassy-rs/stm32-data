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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adc_calibration: Option<chip::AdcCalibration>,
    pub cores: Vec<chip::Core>,
}

pub mod chip {
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct AdcCalibration {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub vrefint: Option<VrefCalibration>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub temp: Option<TempCalibration>,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct VrefCalibration {
        pub vref_mv: u16,
        pub cal_bits: u8,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
    #[serde(tag = "equation", rename_all = "snake_case")]
    pub enum TempCalibration {
        TwoPoint(TwoPointTempCal),
        OnePoint(OnePointTempCal),
        Nominal(NominalTempCal),
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct TwoPointTempCal {
        pub vref_mv: u16,
        pub cal_bits: u8,
        pub cal1_temp_c: i16,
        pub cal2_temp_c: i16,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct OnePointTempCal {
        pub vref_mv: u16,
        pub cal_bits: u8,
        pub cal1_temp_c: i16,
        pub avg_slope_uv_per_c: i32,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct NominalTempCal {
        pub ref_temp_c: i16,
        pub v_ref_mv: u16,
        pub avg_slope_uv_per_c: i32,
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
                #[serde(default, skip_serializing_if = "crate::is_default")]
                pub stop_mode: rcc::StopMode,
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

                #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
                /// Specifies a limit for the stop mode of the peripheral.
                /// E.g. if `StopMode::Stop1` is selected, the peripheral prevents the chip from entering Stop1 mode.
                pub enum StopMode {
                    #[default]
                    /// Peripheral prevents chip from entering Stop1
                    Stop1,
                    /// Peripheral prevents chip from entering Stop2
                    Stop2,
                    /// Peripheral does not prevent chip from entering Stop
                    Standby,
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
