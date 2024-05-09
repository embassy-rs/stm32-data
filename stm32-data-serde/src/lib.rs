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

#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Chip {
    pub name: String,
    pub family: String,
    pub line: String,
    pub die: String,
    pub device_id: u16,
    pub packages: Vec<chip::Package>,
    pub memory: Vec<chip::Memory>,
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
    }

    pub mod memory {
        use serde::{Deserialize, Serialize};

        #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum Kind {
            Flash,
            Ram,
        }

        #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct Settings {
            pub erase_size: u32,
            pub write_size: u32,
            pub erase_value: u8,
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct Doc {
        pub r#type: String,
        pub title: String,
        pub name: String,
        pub url: String,
    }

    #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct Core {
        pub name: String,
        pub peripherals: Vec<core::Peripheral>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub nvic_priority_bits: Option<u8>,
        pub interrupts: Vec<core::Interrupt>,
        pub dma_channels: Vec<core::DmaChannels>,
    }

    pub mod core {
        use serde::{Deserialize, Serialize};

        #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct Peripheral {
            pub name: String,
            #[serde(default)]
            pub address: u32,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub registers: Option<peripheral::Registers>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub rcc: Option<peripheral::Rcc>,
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            pub pins: Vec<peripheral::Pin>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub interrupts: Option<Vec<peripheral::Interrupt>>, // TODO: This should just be a Vec
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
                pub enum StopMode {
                    #[default]
                    Stop1, // Peripheral prevents chip from entering Stop1
                    Stop2,   // Peripheral prevents chip from entering Stop2
                    Standby, // Peripheral does not prevent chip from entering Stop
                }
            }

            #[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
            pub struct Pin {
                pub pin: String,
                pub signal: String,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub af: Option<u8>,
            }

            fn extract_port_and_pin(pin: &str) -> (char, u8) {
                let captures = regex!(r"^P([A-Z])(\d+)(?:_C)?")
                    .captures(pin)
                    .expect("Could not match regex on pin");
                let port = captures
                    .get(1)
                    .expect("Could not extract port")
                    .as_str()
                    .chars()
                    .next()
                    .expect("Empty port");
                let pin_number = captures
                    .get(2)
                    .expect("Could not extract pin number")
                    .as_str()
                    .parse::<u8>()
                    .expect("Could not parse pin number to u8");
                (port, pin_number)
            }

            impl Ord for Pin {
                fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                    let (port_a, pin_number_a) = extract_port_and_pin(&self.pin);
                    let (port_b, pin_number_b) = extract_port_and_pin(&other.pin);

                    if port_a != port_b {
                        port_a.cmp(&port_b)
                    } else if pin_number_a != pin_number_b {
                        pin_number_a.cmp(&pin_number_b)
                    } else {
                        self.signal.cmp(&other.signal)
                    }
                }
            }
            impl PartialOrd for Pin {
                fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                    Some(self.cmp(other))
                }
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
                #[serde(skip_serializing_if = "Option::is_none")]
                pub request: Option<u8>,
            }
        }

        #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct Interrupt {
            pub name: String,
            pub number: u8,
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
