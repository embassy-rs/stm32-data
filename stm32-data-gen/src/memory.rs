use stm32_data_serde::chip::Memory;
use stm32_data_serde::chip::memory::{self, Access, Settings};

use crate::util::new_regex_map;

struct Mem {
    name: String,
    address: u32,
    size: u32,
    access: Option<Access>,
}

fn access(access: &str) -> Access {
    Access {
        read: access.contains('r'),
        write: access.contains('w'),
        execute: access.contains('x'),
    }
}

/// One entry of `data/memory.yaml`: a chip pattern plus the data matched for
/// it. Entries are matched in order and the first match wins, which is why the
/// file is a list rather than a map.
#[derive(serde::Deserialize)]
struct Entry<T> {
    pattern: String,
    variants: Vec<T>,
}

/// A single memory region of a `mems` entry.
#[derive(serde::Deserialize)]
struct MemSpec {
    name: String,
    address: u32,
    #[serde(flatten)]
    size: Size,
    #[serde(default)]
    access: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Size {
    SizeKb(u32),
    SizeBytes(u32),
}

#[derive(serde::Deserialize)]
struct FlashInfo {
    write_size: u32,
    /// Erase region sizes as `(sector size, sector count)` pairs.
    erase_size: Vec<(u32, u32)>,
    /// If erasing flash memory leaves behind 1's: 0xFF; If 0's: 0x00
    erase_value: u8,
}

#[derive(serde::Deserialize)]
struct MemoryYaml {
    mems: Vec<Entry<Vec<MemSpec>>>,
    flash_info: Vec<Entry<FlashInfo>>,
}

pub struct ChipMemories {
    mems: regex_map::RegexMap<Vec<Vec<Mem>>>,
    flash_info: regex_map::RegexMap<Vec<FlashInfo>>,
}

impl ChipMemories {
    pub fn new() -> Self {
        let yaml: MemoryYaml = serde_yaml::from_str(
            &std::fs::read_to_string("data/memory.yaml").expect("failed to read data/memory.yaml"),
        )
        .expect("invalid data/memory.yaml");
        Self {
            mems: new_regex_map(yaml.mems.into_iter().map(|entry| {
                let variants = entry
                    .variants
                    .into_iter()
                    .map(|variant| {
                        variant
                            .into_iter()
                            .map(|spec| Mem {
                                name: spec.name,
                                address: spec.address,
                                size: match spec.size {
                                    Size::SizeKb(kib) => kib * 1024,
                                    Size::SizeBytes(bytes) => bytes,
                                },
                                access: spec.access.as_deref().map(access),
                            })
                            .collect()
                    })
                    .collect();
                (entry.pattern, variants)
            })),
            flash_info: new_regex_map(yaml.flash_info.into_iter().map(|entry| (entry.pattern, entry.variants))),
        }
    }

    pub fn get(&self, chip: &str) -> Vec<Vec<Memory>> {
        let mems_variations = self.mems.get(chip).next().unwrap_or_else(|| {
            panic!("no mems entry matched chip {chip}; add a pattern to `mems` in data/memory.yaml")
        });
        let flash_variations = self.flash_info.get(chip).next().unwrap_or_else(|| {
            panic!("no flash_info entry matched chip {chip}; add a pattern to `flash_info` in data/memory.yaml")
        });

        assert_eq!(
            mems_variations.len(),
            flash_variations.len(),
            "All memory variants must be present in both the mems and the flash info: {chip}"
        );

        mems_variations
            .iter()
            .zip(flash_variations)
            .map(|(mems, flash)| {
                let mut res = Vec::new();

                for mem in mems {
                    if mem.name.starts_with("BANK") {
                        if flash.erase_size.len() == 1 || mem.size <= flash.erase_size[0].0 * flash.erase_size[0].1 {
                            res.push(Memory {
                                name: mem.name.to_string(),
                                address: mem.address,
                                size: mem.size,
                                kind: memory::Kind::Flash,
                                settings: Some(Settings {
                                    write_size: flash.write_size,
                                    erase_size: flash.erase_size[0].0,
                                    erase_value: flash.erase_value,
                                }),
                                access: mem.access,
                            });
                        } else {
                            let mut offs = 0;
                            for (i, &(erase_size, count)) in flash.erase_size.iter().enumerate() {
                                if offs >= mem.size {
                                    break;
                                }
                                let left = mem.size - offs;
                                let mut size = left;
                                if i != flash.erase_size.len() - 1 {
                                    size = size.min(erase_size * count);
                                }
                                #[allow(clippy::redundant_field_names)]
                                res.push(Memory {
                                    name: format!("{}_REGION_{}", mem.name, i + 1),
                                    address: mem.address + offs,
                                    size: size,
                                    kind: memory::Kind::Flash,
                                    settings: Some(Settings {
                                        write_size: flash.write_size,
                                        erase_size: erase_size,
                                        erase_value: flash.erase_value,
                                    }),
                                    access: mem.access,
                                });
                                offs += size;
                            }
                        }
                    } else if mem.name == "OTP" {
                        res.push(Memory {
                            name: mem.name.to_string(),
                            address: mem.address,
                            size: mem.size,
                            kind: memory::Kind::Flash,
                            settings: Some(Settings {
                                write_size: flash.write_size,
                                erase_size: 0,
                                erase_value: flash.erase_value,
                            }),
                            access: mem.access,
                        });
                    } else if mem.name.starts_with("EEPROM") {
                        res.push(Memory {
                            name: mem.name.to_string(),
                            address: mem.address,
                            size: mem.size,
                            kind: memory::Kind::Eeprom,
                            settings: None,
                            access: mem.access,
                        });
                    } else {
                        let mut kind = memory::Kind::Ram;
                        if mem.name.contains("FLASH") || mem.name.contains("AXIICP") {
                            kind = memory::Kind::Flash;
                        }
                        res.push(Memory {
                            name: mem.name.to_string(),
                            address: mem.address,
                            size: mem.size,
                            kind,
                            settings: None,
                            access: mem.access,
                        });
                    }
                }

                res.sort_by_key(|m| (m.address, m.name.clone()));

                res
            })
            .collect()
    }
}
