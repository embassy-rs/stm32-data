use std::fs;

#[derive(Debug, PartialEq)]
struct Memory {
    pub device_id: u16,
    pub names: Vec<String>,
    pub ram: Ram,
    pub flash: Vec<FlashRegion>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Ram {
    pub address: u32,
    pub bytes: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlashRegion {
    pub bank: FlashBank,
    pub name: String,
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

fn splat_names(base: &str, parts: Vec<&str>) -> Vec<String> {
    let mut names = Vec::new();
    for part in parts {
        if part.starts_with("STM32") {
            names.push(base.to_string());
        } else if part.starts_with(&base[5..6]) {
            names.push("STM32".to_string() + part);
        } else {
            let diff = base.len() - part.len();
            names.push((base[..diff]).to_string() + part);
        }
    }

    names
}

fn split_names(str: &str) -> Vec<String> {
    let mut cleaned = Vec::new();
    let mut current_base = None;
    for name in str.split('/') {
        let name = name.split(' ').next().unwrap().trim();
        if name.contains('-') {
            let parts: Vec<_> = name.split('-').collect();
            current_base = parts.first().map(ToString::to_string);
            let splatted = splat_names(&current_base.unwrap(), parts);
            current_base = splatted.first().map(Clone::clone);
            cleaned.extend(splatted);
        } else if name.starts_with("STM32") {
            current_base = Some(name.to_string());
            cleaned.push(name.to_string())
        } else if name.starts_with(&current_base.clone().unwrap()[5..6]) {
            // names.append('STM32' + name)
            cleaned.push("STM32".to_string() + name);
        } else {
            cleaned.push(
                (current_base.clone().unwrap()[0..(current_base.clone().unwrap().len() - name.len())]).to_string()
                    + name,
            )
        }
    }

    cleaned
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
pub struct Memories(Vec<Memory>);

impl Memories {
    pub fn parse() -> anyhow::Result<Self> {
        let mut paths: Vec<_> = glob::glob("sources/cubeprogdb/db/*.xml")
            .unwrap()
            .map(Result::unwrap)
            .collect();
        paths.sort();

        let mut memories = Vec::new();
        let alphabet: Vec<char> = (b'A'..=b'Z').map(|c| c as char).collect();

        for f in paths {
            // println!("Parsing {f:?}");
            let file = fs::read_to_string(f)?;
            let parsed: xml::Root = quick_xml::de::from_str(&file)?;
            // dbg!(&parsed);

            let device_id = parsed.device.device_id;
            let names = split_names(&parsed.device.name);

            let mut ram = None;
            let mut flash = vec![];

            for peripheral in parsed.device.peripherals.peripharal {
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
                    // Use single bank alignment if both single and dual is available
                    let config = peripheral
                        .configuration
                        .iter()
                        .find(|c| c.organization.as_ref().map(|x| x.as_str()).unwrap_or_default() == "Single")
                        .unwrap_or_else(|| peripheral.configuration.first().unwrap());
                    let parameters = config.parameters.as_ref().unwrap();
                    let bank = config.bank.first().unwrap();

                    let flash_bank = match kind {
                        BlockKind::Main => match bank.name.as_ref().map(|x| x.as_str()) {
                            Some("Bank 1") => FlashBank::Bank1,
                            Some("Bank 2") => FlashBank::Bank2,
                            None => FlashBank::Bank1,
                            _ => unimplemented!(),
                        },
                        BlockKind::Otp => FlashBank::Otp,
                    };

                    let erase_value = peripheral.erased_value.unwrap();
                    let write_size = config.allignement.unwrap();
                    flash.extend(bank.field.iter().enumerate().map(|(index, field)| {
                        let name = match kind {
                            BlockKind::Main => format!("MAIN{}", alphabet[index]).to_string(),
                            BlockKind::Otp => "OTP".to_string(),
                        };
                        let bytes = if let Some(occurence) = field.parameters.occurence {
                            occurence * field.parameters.size
                        } else {
                            parameters.size
                        };
                        FlashRegion {
                            bank: flash_bank,
                            name,
                            address: field.parameters.address,
                            bytes,
                            settings: stm32_data_serde::chip::memory::Settings {
                                erase_value,
                                write_size,
                                erase_size: field.parameters.size,
                            },
                        }
                    }));
                }
            }

            memories.push(Memory {
                device_id,
                names,
                ram: ram.unwrap(),
                flash,
            });
        }

        // The chips below are missing from cubeprogdb
        memories.push(Memory {
            device_id: 0,
            names: vec!["STM32F302xD".to_string()],
            ram: Ram {
                address: 0x20000000,
                bytes: 64 * 1024,
            },
            flash: vec![FlashRegion {
                bank: FlashBank::Bank1,
                name: "MAINA".to_string(),
                address: 0x08000000,
                bytes: 384 * 1024,
                settings: stm32_data_serde::chip::memory::Settings {
                    erase_value: 0xFF,
                    write_size: 8,
                    erase_size: 2048,
                },
            }],
        });

        memories.push(Memory {
            device_id: 0,
            names: vec!["STM32F303xD".to_string()],
            ram: Ram {
                address: 0x20000000,
                bytes: 80 * 1024,
            },
            flash: vec![FlashRegion {
                bank: FlashBank::Bank1,
                name: "MAINA".to_string(),
                address: 0x08000000,
                bytes: 384 * 1024,
                settings: stm32_data_serde::chip::memory::Settings {
                    erase_value: 0xFF,
                    write_size: 8,
                    erase_size: 2048,
                },
            }],
        });

        memories.push(Memory {
            device_id: 0,
            names: vec!["STM32L100x6".to_string()],
            ram: Ram {
                address: 0x20000000,
                bytes: 32 * 1024,
            },
            flash: vec![FlashRegion {
                bank: FlashBank::Bank1,
                name: "MAINA".to_string(),
                address: 0x08000000,
                bytes: 4 * 1024,
                settings: stm32_data_serde::chip::memory::Settings {
                    erase_value: 0xFF,
                    write_size: 4,
                    erase_size: 256,
                },
            }],
        });

        Ok(Self(memories))
    }

    fn lookup_chip(&self, chip_name: &str) -> &Memory {
        for each in &self.0 {
            for name in &each.names {
                if is_chip_name_match(name, chip_name) {
                    return each;
                }
            }
        }
        panic!("could not find memory information for {chip_name}");
    }

    pub fn determine_ram_size(&self, chip_name: &str) -> u32 {
        self.lookup_chip(chip_name).ram.bytes
    }

    pub fn determine_flash_regions(&self, chip_name: &str) -> &[FlashRegion] {
        self.lookup_chip(chip_name).flash.as_slice()
    }

    pub fn determine_device_id(&self, chip_name: &str) -> u16 {
        self.lookup_chip(chip_name).device_id
    }
}

fn is_chip_name_match(pattern: &str, chip_name: &str) -> bool {
    let mut chip_name = chip_name.replace("STM32F479", "STM32F469"); // F479 is missing, it's the same as F469.
    chip_name = chip_name.replace("STM32G050", "STM32G051"); // same...
    chip_name = chip_name.replace("STM32G060", "STM32G061"); // same...
    chip_name = chip_name.replace("STM32G070", "STM32G071"); // same...
    chip_name = chip_name.replace("STM32G0B0", "STM32G0B1"); // same...
    chip_name = chip_name.replace("STM32G4A", "STM32G49"); // same...
    chip_name = chip_name.replace("STM32L422", "STM32L412"); // same...
    chip_name = chip_name.replace("STM32WB30", "STM32WB35"); // same...
    let pattern = pattern.replace('x', ".");
    regex::Regex::new(&pattern).unwrap().is_match(&chip_name)
}
