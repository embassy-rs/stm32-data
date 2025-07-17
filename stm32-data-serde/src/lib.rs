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
    pub cores: Vec<chip::Core>,
}

pub mod chip {
    use serde::{Deserialize, Serialize};

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
                    Mux(Field),
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
            pub struct DmaChannel {
                pub signal: String,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub dma: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub channel: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub dmamux: Option<String>,
                #[serde(skip_serializing_if = "Vec::is_empty")]
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
}
