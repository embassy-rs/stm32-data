use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;

#[derive(Debug, PartialEq)]
pub struct Memory {
    pub device_id: u16,
    pub ram: Ram,
    pub flash_size: u32,
    pub flash_regions: Vec<FlashRegion>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ram {
    pub address: u32,
    pub bytes: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlashRegion {
    pub bank: FlashBank,
    pub address: u32,
    pub bytes: u32,
    pub settings: stm32_data_serde::chip::memory::Settings,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FlashBank {
    Bank1,
    Bank2,
    Otp,
}

mod xml {
    use serde::Deserialize;

    pub fn from_hex<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        D: serde::Deserializer<'de>,
        T: num::Num,
        T::FromStrRadixErr: std::fmt::Display,
    {
        use serde::de::Error;
        let s: &str = Deserialize::deserialize(deserializer)?;
        let s = s.trim();
        let (prefix, num) = s.split_at(2);
        if prefix != "0x" && prefix != "0X" {
            panic!("no hex prefix");
        }
        T::from_str_radix(num, 16).map_err(D::Error::custom)
    }

    pub fn opt_from_hex<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
    where
        D: serde::Deserializer<'de>,
        T: num::Num,
        T::FromStrRadixErr: std::fmt::Display,
    {
        Ok(Some(from_hex(deserializer)?))
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct Root {
        #[serde(rename = "Device")]
        pub device: root::Device,
    }

    mod root {
        use serde::Deserialize;

        use super::from_hex;

        #[derive(Debug, Deserialize, PartialEq)]
        pub struct Device {
            #[serde(rename = "DeviceID", deserialize_with = "from_hex")]
            pub device_id: u16,
            #[serde(rename = "Name")]
            pub name: String,
            #[serde(rename = "Peripherals")]
            pub peripherals: device::Peripherals,
        }

        mod device {
            use serde::Deserialize;

            #[derive(Debug, Deserialize, PartialEq)]
            pub struct Peripherals {
                #[serde(rename = "Peripheral")]
                pub peripharal: Vec<peripherals::Peripheral>,
            }

            mod peripherals {
                use serde::Deserialize;

                use super::super::super::opt_from_hex;

                #[derive(Debug, Deserialize, PartialEq)]
                pub struct Peripheral {
                    #[serde(rename = "Name")]
                    // pub name: peripheral::Name,
                    pub name: String,
                    #[serde(rename = "ErasedValue", deserialize_with = "opt_from_hex", default)]
                    pub erased_value: Option<u8>,
                    #[serde(rename = "Configuration", default)]
                    pub configuration: Vec<peripheral::Configuration>,
                }

                mod peripheral {
                    use serde::Deserialize;

                    use super::super::super::super::opt_from_hex;

                    #[derive(Debug, Deserialize, PartialEq)]
                    pub struct Configuration {
                        #[serde(rename = "Parameters", default)]
                        pub parameters: Option<configuration::Parameters>,
                        #[serde(rename = "Organization", default)]
                        pub organization: Option<String>,
                        #[serde(rename = "Allignement", deserialize_with = "opt_from_hex", default)]
                        pub allignement: Option<u32>,
                        #[serde(rename = "Bank")]
                        pub bank: Vec<configuration::Bank>,
                    }

                    mod configuration {
                        use serde::Deserialize;

                        use super::super::super::super::super::{from_hex, opt_from_hex};

                        #[derive(Debug, Deserialize, PartialEq)]
                        pub struct Parameters {
                            #[serde(deserialize_with = "from_hex")]
                            pub address: u32,
                            #[serde(deserialize_with = "from_hex")]
                            pub size: u32,
                            #[serde(deserialize_with = "opt_from_hex", default)]
                            pub occurence: Option<u32>,
                        }

                        #[derive(Debug, Deserialize, PartialEq)]
                        pub struct Bank {
                            #[serde(default)]
                            pub name: Option<String>,
                            #[serde(rename = "Field", default)]
                            pub field: Vec<bank::Field>,
                        }

                        mod bank {
                            use serde::Deserialize;

                            #[derive(Debug, Deserialize, PartialEq)]
                            pub struct Field {
                                #[serde(rename = "Parameters")]
                                pub parameters: super::Parameters,
                            }
                        }
                    }
                }
            }
        }
    }
}
pub struct Memories(HashMap<u16, Memory>);

impl Memories {
    pub fn parse() -> anyhow::Result<Self> {
        let mut paths: Vec<_> = glob::glob("sources/cubeprogdb/db/*.xml")
            .unwrap()
            .map(Result::unwrap)
            .collect();
        paths.sort();

        let mut memories = HashMap::new();

        for f in paths {
            // println!("Parsing {f:?}");
            let file = fs::read_to_string(f)?;
            let parsed: xml::Root = quick_xml::de::from_str(&file)?;
            // dbg!(&parsed);

            let device_id = parsed.device.device_id;

            let mut ram = None;
            let mut flash_size = None;
            let mut flash_regions = vec![];

            for mut peripheral in parsed.device.peripherals.peripharal {
                if peripheral.name == "Embedded SRAM" && ram.is_none() {
                    let config = peripheral.configuration.first().unwrap();
                    let parameters = config.parameters.as_ref().unwrap();
                    ram = Some(Ram {
                        address: parameters.address,
                        bytes: parameters.size,
                    });
                }

                enum BlockKind {
                    Main,
                    Otp,
                }
                let kind = match peripheral.name.as_str() {
                    "Embedded Flash" => Some(BlockKind::Main),
                    "OTP" => Some(BlockKind::Otp),
                    _ => None,
                };

                if let Some(kind) = kind {
                    peripheral.configuration.sort_by(|a, b| {
                        // Prefer largest size
                        let ordering = b
                            .parameters
                            .as_ref()
                            .unwrap()
                            .size
                            .partial_cmp(&a.parameters.as_ref().unwrap().size)
                            .unwrap();

                        // ... then prefer single ordering over dual
                        if ordering == Ordering::Equal {
                            // Possible values are Single and Dual
                            b.organization.partial_cmp(&a.organization).unwrap()
                        } else {
                            ordering
                        }
                    });
                    let config = peripheral.configuration.first().unwrap();

                    if flash_size.is_none() {
                        let parameters = config.parameters.as_ref().unwrap();

                        flash_size = Some(parameters.size);
                    }

                    for bank in config.bank.iter() {
                        let flash_bank = match kind {
                            BlockKind::Main => match bank.name.as_ref().map(|x| x.as_str()) {
                                Some("Bank 1") => Some(FlashBank::Bank1),
                                Some("Bank 2") => Some(FlashBank::Bank2),
                                Some("EEPROM1") => None,
                                Some("EEPROM2") => None,
                                None => {
                                    assert_eq!(1, config.bank.len());
                                    Some(FlashBank::Bank1)
                                }
                                Some(other) => unimplemented!("Unsupported flash bank {}", other),
                            },
                            BlockKind::Otp => Some(FlashBank::Otp),
                        };

                        if let Some(flash_bank) = flash_bank {
                            let erase_value = peripheral.erased_value.unwrap();
                            let write_size = config.allignement.unwrap();
                            flash_regions.extend(bank.field.iter().map(|field| FlashRegion {
                                bank: flash_bank,
                                address: field.parameters.address,
                                bytes: field.parameters.occurence.unwrap() * field.parameters.size,
                                settings: stm32_data_serde::chip::memory::Settings {
                                    erase_value,
                                    write_size,
                                    erase_size: field.parameters.size,
                                },
                            }));
                        }
                    }
                }
            }

            memories.insert(
                device_id,
                Memory {
                    device_id,
                    ram: ram.unwrap(),
                    flash_size: flash_size.unwrap_or_default(),
                    flash_regions,
                },
            );
        }

        Ok(Self(memories))
    }

    pub fn get(&self, die: &str) -> &Memory {
        assert!(die.starts_with("DIE"));
        let device_id = u16::from_str_radix(&die[3..], 16).unwrap();

        self.0.get(&device_id).unwrap()
    }
}
