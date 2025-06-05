use serde::Deserialize;
use stm32_data_macros::EnumDebug;

pub mod ir {
    use super::*;

    #[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
    pub struct IR {
        pub blocks: Vec<Block>,
        pub fieldsets: Vec<FieldSet>,
        pub enums: Vec<Enum>,
    }

    impl IR {
        pub fn from_chiptool(ir: chiptool::ir::IR) -> Self {
            let mut blocks: Vec<Block> = ir
                .blocks
                .iter()
                .map(|(name, block)| {
                    let items = block
                        .items
                        .iter()
                        .map(|item| BlockItem {
                            name: item.name.clone(),
                            description: item.description.clone(),
                            array: item.array.as_ref().map(|array| match &array {
                                chiptool::ir::Array::Regular(regular_array) => Array::Regular(RegularArray {
                                    len: regular_array.len,
                                    stride: regular_array.stride,
                                }),
                                chiptool::ir::Array::Cursed(cursed_array) => Array::Cursed(CursedArray {
                                    offsets: cursed_array.offsets.clone(),
                                }),
                            }),
                            byte_offset: item.byte_offset,
                            inner: match &item.inner {
                                chiptool::ir::BlockItemInner::Block(block) => BlockItemInner::Block(BlockItemBlock {
                                    block: block.block.clone(),
                                }),
                                chiptool::ir::BlockItemInner::Register(register) => {
                                    BlockItemInner::Register(Register {
                                        access: match register.access {
                                            chiptool::ir::Access::Read => Access::Read,
                                            chiptool::ir::Access::ReadWrite => Access::ReadWrite,
                                            chiptool::ir::Access::Write => Access::Write,
                                        },
                                        bit_size: register.bit_size,
                                        fieldset: register
                                            .fieldset
                                            .as_ref()
                                            .map(|fieldset| fieldset.strip_prefix("regs::").unwrap().to_string()),
                                    })
                                }
                            },
                        })
                        .collect();

                    #[allow(clippy::redundant_field_names)]
                    Block {
                        name: name.to_string(),
                        items: items,
                        extends: block.extends.clone(),
                        description: block.description.clone(),
                    }
                })
                .collect();

            blocks.sort_by_key(|b| b.name.clone());

            let mut fieldsets: Vec<FieldSet> = ir
                .fieldsets
                .iter()
                .map(|(name, fieldset)| {
                    let fields = fieldset
                        .fields
                        .iter()
                        .map(|field| Field {
                            name: field.name.clone(),
                            description: field.description.clone(),
                            bit_offset: match &field.bit_offset {
                                chiptool::ir::BitOffset::Regular(offset) => {
                                    BitOffset::Regular(RegularBitOffset { offset: *offset })
                                }
                                chiptool::ir::BitOffset::Cursed(ranges) => {
                                    BitOffset::Cursed(CursedBitOffset { ranges: ranges.clone() })
                                }
                            },
                            bit_size: field.bit_size,
                            array: field.array.as_ref().map(|array| match &array {
                                chiptool::ir::Array::Regular(regular_array) => Array::Regular(RegularArray {
                                    len: regular_array.len,
                                    stride: regular_array.stride,
                                }),
                                chiptool::ir::Array::Cursed(cursed_array) => Array::Cursed(CursedArray {
                                    offsets: cursed_array.offsets.clone(),
                                }),
                            }),
                            enumm: field
                                .enumm
                                .as_ref()
                                .map(|fieldset| fieldset.strip_prefix("vals::").unwrap().to_string()),
                        })
                        .collect();

                    #[allow(clippy::redundant_field_names)]
                    FieldSet {
                        name: name.strip_prefix("regs::").unwrap().to_owned(),
                        fields: fields,
                        extends: fieldset.extends.clone(),
                        description: fieldset.description.clone(),
                        bit_size: fieldset.bit_size,
                    }
                })
                .collect();

            fieldsets.sort_by_key(|f| f.name.clone());

            let mut enums: Vec<Enum> = ir
                .enums
                .iter()
                .map(|(name, enumm)| {
                    let variants = enumm
                        .variants
                        .iter()
                        .map(|variant| EnumVariant {
                            name: variant.name.clone(),
                            description: variant.description.clone(),
                            value: variant.value,
                        })
                        .collect();

                    #[allow(clippy::redundant_field_names)]
                    Enum {
                        name: name.strip_prefix("vals::").unwrap().to_owned(),
                        description: enumm.description.clone(),
                        bit_size: enumm.bit_size,
                        variants: variants,
                    }
                })
                .collect();

            enums.sort_by_key(|e| e.name.clone());

            Self {
                blocks,
                fieldsets,
                enums,
            }
        }
    }

    #[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
    pub struct Block {
        pub name: String,
        pub extends: Option<String>,

        pub description: Option<String>,
        pub items: Vec<BlockItem>,
    }

    // Notice:
    // BlockItem has custom Debug implement,
    // when modify the struct, make sure Debug impl reflect the change.
    #[derive(Eq, PartialEq, Clone, Deserialize)]
    pub struct BlockItem {
        pub name: String,
        pub description: Option<String>,

        pub array: Option<Array>,
        pub byte_offset: u32,

        pub inner: BlockItemInner,
    }

    // Notice:
    // Debug implement AFFECT OUTPUT METAPAC, modify with caution
    impl std::fmt::Debug for BlockItem {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("BlockItem")
                .field("name", &self.name)
                .field("description", &self.description)
                .field("array", &self.array)
                .field("byte_offset", &format_args!("{:#x}", self.byte_offset))
                .field("inner", &self.inner)
                .finish()
        }
    }

    #[derive(EnumDebug, Eq, PartialEq, Clone, Deserialize)]
    pub enum BlockItemInner {
        Block(BlockItemBlock),
        Register(Register),
    }

    #[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
    pub struct Register {
        pub access: Access,
        pub bit_size: u32,
        pub fieldset: Option<String>,
    }

    #[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
    pub struct BlockItemBlock {
        pub block: String,
    }

    #[derive(EnumDebug, Eq, PartialEq, Clone, Deserialize)]
    pub enum Access {
        ReadWrite,
        Read,
        Write,
    }

    #[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
    pub struct FieldSet {
        pub name: String,
        pub extends: Option<String>,

        pub description: Option<String>,
        pub bit_size: u32,
        pub fields: Vec<Field>,
    }

    #[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
    pub struct Field {
        pub name: String,
        pub description: Option<String>,

        pub bit_offset: BitOffset,
        pub bit_size: u32,
        pub array: Option<Array>,
        pub enumm: Option<String>,
    }

    #[derive(EnumDebug, Eq, PartialEq, Clone, Deserialize)]
    pub enum Array {
        Regular(RegularArray),
        Cursed(CursedArray),
    }

    #[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
    pub struct RegularArray {
        pub len: u32,
        pub stride: u32,
    }

    #[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
    pub struct CursedArray {
        pub offsets: Vec<u32>,
    }

    #[derive(EnumDebug, Eq, PartialEq, Clone, Deserialize)]
    pub enum BitOffset {
        Regular(RegularBitOffset),
        Cursed(CursedBitOffset),
    }

    #[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
    pub struct RegularBitOffset {
        pub offset: u32,
    }

    #[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
    pub struct CursedBitOffset {
        pub ranges: Vec<core::ops::RangeInclusive<u32>>,
    }

    #[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
    pub struct Enum {
        pub name: String,
        pub description: Option<String>,
        pub bit_size: u32,
        pub variants: Vec<EnumVariant>,
    }

    #[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
    pub struct EnumVariant {
        pub name: String,
        pub description: Option<String>,
        pub value: u64,
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
pub struct Chip {
    pub name: String,
    pub family: String,
    pub line: String,
    pub cores: Vec<Core>,
    pub memory: Vec<Vec<MemoryRegion>>,
    pub packages: Vec<Package>,
}

// Notice:
// MemoryRegion has custom Debug implement,
// when modify the struct, make sure Debug impl reflect the change.
#[derive(Eq, PartialEq, Clone, Deserialize)]
pub struct MemoryRegion {
    pub name: String,
    pub kind: MemoryRegionKind,
    pub address: u32,
    pub size: u32,
    pub settings: Option<FlashSettings>,
}

// Notice:
// Debug implement AFFECT OUTPUT METAPAC, modify with caution
impl std::fmt::Debug for MemoryRegion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryRegion")
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("address", &format_args!("{:#x}", self.address))
            .field("size", &self.size)
            .field("settings", &self.settings)
            .finish()
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
pub struct FlashSettings {
    pub erase_size: u32,
    pub write_size: u32,
    pub erase_value: u8,
}

#[derive(EnumDebug, Eq, PartialEq, Clone, Deserialize)]
pub enum MemoryRegionKind {
    #[serde(rename = "flash")]
    Flash,
    #[serde(rename = "ram")]
    Ram,
    #[serde(rename = "eeprom")]
    Eeprom,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
pub struct Core {
    pub name: String,
    pub peripherals: Vec<Peripheral>,
    #[serde(default)]
    pub nvic_priority_bits: Option<u8>,
    pub interrupts: Vec<Interrupt>,
    pub dma_channels: Vec<DmaChannel>,
    pub pins: Vec<Pin>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
pub struct Interrupt {
    pub name: String,
    pub number: u32,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
pub struct Package {
    pub name: String,
    pub package: String,
}

// Notice:
// Peripheral has custom Debug implement,
// when modify struct, make sure Debug impl reflect the change.
#[derive(Eq, PartialEq, Clone, Deserialize)]
pub struct Peripheral {
    pub name: String,
    pub address: u64,
    #[serde(default)]
    pub registers: Option<PeripheralRegisters>,
    #[serde(default)]
    pub rcc: Option<PeripheralRcc>,
    #[serde(default)]
    pub pins: Vec<PeripheralPin>,
    #[serde(default)]
    pub dma_channels: Vec<PeripheralDmaChannel>,
    #[serde(default)]
    pub interrupts: Vec<PeripheralInterrupt>,
}

// Notice:
// Debug implement AFFECT OUTPUT METAPAC, modify with caution
impl std::fmt::Debug for Peripheral {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Peripheral")
            .field("name", &self.name)
            .field("address", &format_args!("{:#x}", self.address))
            .field("registers", &self.registers)
            .field("rcc", &self.rcc)
            .field("pins", &self.pins)
            .field("dma_channels", &self.dma_channels)
            .field("interrupts", &self.interrupts)
            .finish()
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
pub struct PeripheralInterrupt {
    pub signal: String,
    pub interrupt: String,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
pub struct PeripheralRcc {
    pub bus_clock: String,
    pub kernel_clock: PeripheralRccKernelClock,
    #[serde(default)]
    pub enable: Option<PeripheralRccRegister>,
    #[serde(default)]
    pub reset: Option<PeripheralRccRegister>,
    #[serde(default)]
    pub stop_mode: StopMode,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
#[serde(untagged)]
pub enum PeripheralRccKernelClock {
    Clock(String),
    Mux(PeripheralRccRegister),
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
pub struct PeripheralRccRegister {
    pub register: String,
    pub field: String,
}

#[derive(EnumDebug, Eq, PartialEq, Clone, Deserialize, Default)]
pub enum StopMode {
    #[default]
    Stop1, // Peripheral prevents chip from entering Stop1
    Stop2,   // Peripheral prevents chip from entering Stop2
    Standby, // Peripheral does not prevent chip from entering Stop
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
pub struct PeripheralPin {
    pub pin: String,
    pub signal: String,
    pub af: Option<u8>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
pub struct Pin {
    pub name: String,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
pub struct DmaChannel {
    pub name: String,
    pub dma: String,
    pub channel: u32,
    pub dmamux: Option<String>,
    pub dmamux_channel: Option<u32>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Hash)]
pub struct PeripheralDmaChannel {
    pub signal: String,
    pub channel: Option<String>,
    pub dmamux: Option<String>,
    pub dma: Option<String>,
    pub request: Option<u32>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Hash)]
pub struct PeripheralRegisters {
    pub kind: String,
    pub version: String,
    pub block: String,
    #[serde(default)]
    pub ir: String,
}
