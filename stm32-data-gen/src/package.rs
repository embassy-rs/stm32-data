use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Ok;
use itertools::Itertools;
use log::trace;
use serde::{Deserialize, Serialize};
use stm32_data_serde::chip::PackagePin;
use stm32_data_serde::chip::core::{DmaChannels, peripheral};

use crate::chips::xml::PinSignal;
use crate::chips::{Chip, ChipGroup, merge_pins, xml};
use crate::dma::ChipDma;
use crate::gpio_af::{Af, clean_pin, pin_matches};
use crate::interrupts::ChipInterrupts;
use crate::package::schema::pinout::Characteristics;
use crate::package::schema::{dma, exti, interrupts, peripherals, pinout};
use crate::util::{entry_or, occupied_entry_or};

#[derive(Serialize, Deserialize)]
pub struct Package {
    pub devices: Devices,
}

#[derive(Serialize, Deserialize)]
pub struct Devices {
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "family")]
    pub families: Vec<Family>,
}

#[derive(Serialize, Deserialize)]
pub struct Family {
    #[serde(default, rename = "@Dfamily")]
    pub family: String,
    #[serde(default, rename = "@Dvendor")]
    pub vendor: String,
    #[serde(default, rename = "processor")]
    pub processors: Vec<Processor>,
    #[serde(default, rename = "book")]
    pub books: Vec<Book>,
    #[serde(default)]
    pub description: String,
    #[serde(default, rename = "feature")]
    pub features: Vec<Feature>,
    #[serde(default, rename = "environment")]
    pub environments: Vec<Environment>,
    #[serde(default, rename = "subFamily")]
    pub sub_families: Vec<SubFamily>,
}

#[derive(Serialize, Deserialize)]
pub struct SubFamily {
    #[serde(rename = "@DsubFamily")]
    pub sub_family: String,
    #[serde(default, rename = "memory")]
    pub memories: Vec<Memory>,
    #[serde(default, rename = "book")]
    pub books: Vec<Book>,
    #[serde(default, rename = "feature")]
    pub features: Vec<Feature>,
    #[serde(default, rename = "environment")]
    pub environments: Vec<Environment>,
    #[serde(default, rename = "device")]
    pub devices: Vec<Device>,
}

/// Corresponds to one `ChipGroup`
#[derive(Serialize, Deserialize)]
pub struct Device {
    #[serde(rename = "@Dname")]
    pub name: String,
    #[serde(default, rename = "compile")]
    pub compiles: Vec<Compile>,
    #[serde(default, rename = "memory")]
    pub memories: Vec<Memory>,
    #[serde(default, rename = "algorithm")]
    pub algorithms: Vec<Algorithm>,
    #[serde(default, rename = "book")]
    pub books: Vec<Book>,
    #[serde(default)]
    pub debug: Option<Debug>,
    #[serde(default)]
    pub flashinfo: Option<Flashinfo>,
    #[serde(default, rename = "feature")]
    pub features: Vec<Feature>,
    #[serde(default, rename = "environment")]
    pub environments: Vec<Environment>,
    #[serde(default, rename = "variant")]
    pub variants: Vec<Variant>,
}

#[derive(Serialize, Deserialize)]
pub struct Variant {
    #[serde(rename = "@Dvariant")]
    pub variant: String,
    #[serde(default, rename = "feature")]
    pub features: Vec<Feature>,
    #[serde(default, rename = "environment")]
    pub environments: Vec<Environment>,
}

#[derive(Serialize, Deserialize)]
pub struct Environment {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(default, rename = "device")]
    pub device: STDevice,
}

#[derive(Serialize, Deserialize, Default)]
pub struct STDevice {
    #[serde(default)]
    pub descriptors: Descriptors,
    #[serde(default, rename = "extra-attributes")]
    pub attributes: Attributes,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Descriptors {
    #[serde(rename = "descriptor")]
    pub descriptors: Vec<Descriptor>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Descriptor {
    #[serde(rename = "@schemaType")]
    pub schema_type: String,
    #[serde(rename = "@path")]
    pub path: String,
    #[serde(rename = "@schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "@version")]
    pub version: String,
}

impl Descriptor {
    fn as_path(&self) -> &Path {
        Path::new(&self.path)
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct Attributes {
    #[serde(rename = "extra-attribute")]
    pub attributes: Vec<Attribute>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Attribute {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@value")]
    pub value: String,
}

#[derive(Serialize, Deserialize)]
pub struct Processor {
    #[serde(default, rename = "@Dcore")]
    pub core: String,
    #[serde(default, rename = "@DcoreVersion")]
    pub core_version: String,
    #[serde(default, rename = "@Dfpu")]
    pub fpu: String,
    #[serde(default, rename = "@Dmpu")]
    pub mpu: String,
    #[serde(default, rename = "@Ddsp")]
    pub dsp: String,
    #[serde(default, rename = "@Dtz")]
    pub tz: String,
    #[serde(default, rename = "@Dendian")]
    pub endian: String,
    #[serde(default, rename = "@Dclock")]
    pub clock: String,
}

#[derive(Serialize, Deserialize)]
pub struct Memory {
    #[serde(default, rename = "@name")]
    pub name: String,
    #[serde(default, rename = "@access")]
    pub access: String,
    #[serde(default, rename = "@start")]
    pub start: String,
    #[serde(default, rename = "@size")]
    pub size: String,
    #[serde(default, rename = "@uninit")]
    pub uninit: String,
    #[serde(default, rename = "@default")]
    pub default: String,
    #[serde(default, rename = "@startup")]
    pub startup: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Feature {
    #[serde(rename = "@type")]
    pub feature_type: String,
    #[serde(default, rename = "@name")]
    pub name: String,
    #[serde(default, rename = "@n")]
    pub n: String,
    #[serde(default, rename = "@m")]
    pub m: String,
}

#[derive(Serialize, Deserialize)]
pub struct Compile {
    #[serde(rename = "@header")]
    pub header: String,
    #[serde(rename = "@define")]
    pub define: String,
}

#[derive(Serialize, Deserialize)]
pub struct Algorithm {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@start")]
    pub start: String,
    #[serde(rename = "@size")]
    pub size: String,
    #[serde(rename = "@RAMstart")]
    pub ramstart: String,
    #[serde(rename = "@RAMsize")]
    pub ramsize: String,
    #[serde(rename = "@default")]
    pub default: String,
}

#[derive(Serialize, Deserialize)]
pub struct Book {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@title")]
    pub title: String,
}

#[derive(Serialize, Deserialize)]
pub struct Debug {
    #[serde(rename = "@svd")]
    pub svd: String,
    #[serde(rename = "@__ap")]
    pub _ap: String,
}

#[derive(Serialize, Deserialize)]
pub struct Flashinfo {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@start")]
    pub start: String,
    #[serde(rename = "@pagesize")]
    pub pagesize: String,
    #[serde(rename = "$text")]
    pub text: Option<String>,
    pub block: Block,
}

#[derive(Serialize, Deserialize)]
pub struct Block {
    #[serde(rename = "@count")]
    pub count: String,
    #[serde(rename = "@size")]
    pub size: String,
}

mod schema {
    pub mod pinout {
        use serde::{Deserialize, Serialize};

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct File {
            #[serde(default, rename = "schema_version")]
            pub schema_version: String,
            #[serde(default, rename = "characteristics")]
            pub characteristics: Characteristics,
            #[serde(default, rename = "pin_type_description")]
            pub pin_type_description: PinTypeDescription,
            #[serde(default, rename = "io_structure_type_description")]
            pub io_structure_type_description: IoStructureTypeDescription,
            #[serde(default, rename = "io_structure_options_description")]
            pub io_structure_options_description: IoStructureOptionsDescription,
            #[serde(default, rename = "package_pins")]
            pub package_pins: Vec<String>,
            #[serde(default, rename = "signals")]
            pub signals: Vec<Signal>,
            #[serde(default, rename = "bonds")]
            pub bonds: Vec<Bond>,
            #[serde(default, rename = "version")]
            pub version: String,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct Characteristics {
            #[serde(default, rename = "package_name")]
            pub package_name: String,
            #[serde(default, rename = "package_type")]
            pub package_type: String,
            #[serde(default, rename = "die_name")]
            pub die_name: String,
            #[serde(default, rename = "NbIOs")]
            pub nb_ios: i64,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct PinTypeDescription {
            #[serde(default)]
            pub s: String,
            #[serde(default, rename = "I/O")]
            pub i_o: String,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct IoStructureTypeDescription {
            #[serde(default, rename = "RST")]
            pub rst: String,
            #[serde(default, rename = "FT")]
            pub ft: String,
            #[serde(default, rename = "TT")]
            pub tt: String,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct IoStructureOptionsDescription {
            #[serde(default, rename = "a")]
            pub a: String,
            #[serde(default, rename = "f")]
            pub f: String,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct Signal {
            #[serde(default, rename = "name")]
            pub name: String,
            #[serde(default, rename = "instance")]
            pub instance: String,
            #[serde(default, rename = "die_pad")]
            pub die_pad: String,
            #[serde(default, rename = "function")]
            pub function: Function,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct Function {
            #[serde(default, rename = "type")]
            pub type_field: String,
            #[serde(default, rename = "id")]
            pub id: Option<String>,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct Bond {
            #[serde(default, rename = "die_pad")]
            pub die_pad: String,
            #[serde(default, rename = "position")]
            pub position: String,
            #[serde(default, rename = "sharing")]
            pub sharing: Option<Sharing>,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct Sharing {
            #[serde(default, rename = "signals")]
            pub signals: Vec<String>,
        }
    }

    pub mod peripherals {
        use serde::{Deserialize, Serialize};

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct File {
            #[serde(default, rename = "analogInterconnections")]
            pub analog_interconnections: Vec<AnalogInterconnections>,

            #[serde(default)]
            pub peripherals: Vec<Peripheral>,

            #[serde(default)]
            pub schema_version: String,

            #[serde(default)]
            pub version: String,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct Peripheral {
            #[serde(default, rename = "busMapping")]
            pub bus_mapping: Vec<BusMapping>,

            #[serde(default, rename = "digitalName")]
            pub digital_name: String,

            #[serde(default, rename = "entityType")]
            pub entity_type: String,

            #[serde(default)]
            pub interconnect: Vec<Interconnect>,

            #[serde(default)]
            pub name: String,

            #[serde(default, rename = "peripheralType")]
            pub peripheral_type: String,

            #[serde(default, rename = "peripheralVersionNum")]
            pub peripheral_version_num: f64,

            #[serde(default)]
            pub pinout_signals: Vec<PinoutSignals>,

            #[serde(default)]
            pub protocols: Vec<Protocol>,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct Interconnect {
            #[serde(default)]
            pub id: String,

            #[serde(default)]
            pub instance: String,

            #[serde(default)]
            pub source: String,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct Protocol {
            #[serde(default)]
            pub modes: Vec<serde_json::Value>,

            #[serde(default)]
            pub signals: Vec<Signal>,

            #[serde(default, rename = "type")]
            pub typ: String,

            #[serde(default)]
            pub version: String,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct Signal {
            #[serde(default)]
            pub signal: String,

            #[serde(default, rename = "type")]
            pub typ: String,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct PinoutSignals {
            #[serde(default)]
            pub id: String,

            #[serde(default)]
            pub name: String,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct BusMapping {
            #[serde(default)]
            pub connections: Vec<serde_json::Value>,

            #[serde(default)]
            pub mode: String,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct AnalogInterconnections {
            #[serde(default, rename = "digitalName")]
            pub digital_name: String,

            #[serde(default)]
            pub inputs: Vec<Inputs>,

            #[serde(default, rename = "instanceName")]
            pub instance_name: String,

            #[serde(default)]
            pub name: String,

            #[serde(default)]
            pub outputs: Vec<serde_json::Value>,

            #[serde(default)]
            pub version: String,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct Inputs {
            #[serde(default, rename = "connectionInstance")]
            pub connection_instance: String,

            #[serde(default, rename = "connectionPin")]
            pub connection_pin: String,

            #[serde(default, rename = "connectionUser")]
            pub connection_user: String,

            #[serde(default, rename = "internalSignal")]
            pub internal_signal: String,
        }
    }

    pub mod interrupts {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize)]
        pub struct File {
            #[serde(default)]
            pub instances: Vec<Instance>,

            #[serde(default)]
            pub schema_version: String,

            #[serde(default)]
            pub version: String,
        }

        #[derive(Debug, Serialize, Deserialize)]
        pub struct Instance {
            #[serde(default)]
            pub core_type: String,

            #[serde(default)]
            pub cpu_instance: String,

            #[serde(default)]
            pub instance: String,

            #[serde(default)]
            pub interrupts: Vec<Interrupt>,

            #[serde(default)]
            pub priority_bits: i64,

            #[serde(default)]
            pub priority_grouping: bool,
        }

        #[derive(Debug, Serialize, Deserialize)]
        pub struct Peripheral {
            #[serde(default)]
            pub name: String,
        }

        #[derive(Debug, Serialize, Deserialize)]
        pub struct Interrupt {
            #[serde(default)]
            pub access: Access,

            #[serde(default)]
            pub acronyms: Vec<serde_json::Value>,

            #[serde(default)]
            pub description: String,

            #[serde(default, rename = "instances")]
            pub peripherals: Vec<Peripheral>,

            #[serde(default)]
            pub name: String,

            #[serde(default)]
            pub priority: i64,

            #[serde(default)]
            pub settable: bool,

            #[serde(default, rename = "type")]
            pub typ: String,

            #[serde(default)]
            pub used_at_reset: bool,
        }

        #[derive(Debug, Serialize, Deserialize, Default)]
        pub struct Access {
            #[serde(default)]
            pub non_secure: bool,

            #[serde(default)]
            pub secure: bool,
        }
    }

    pub mod exti {
        use serde::{Deserialize, Serialize};

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct File {
            pub lines: Vec<Line>,
            #[serde(default, rename = "schema_version")]
            pub schema_version: String,
            #[serde(default)]
            pub version: String,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct Line {
            #[serde(default)]
            pub configurable: bool,
            #[serde(default, rename = "connected_nvic")]
            pub connected_nvic: String,
            #[serde(default)]
            pub interconnect: Vec<Interconnect>,
            #[serde(default, rename = "line_id")]
            pub line_id: String,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct Interconnect {
            #[serde(default)]
            pub instance: String,
            #[serde(default)]
            pub pin: String,
            #[serde(default, rename = "type")]
            pub typ: String,
            #[serde(default)]
            pub source: String,
        }
    }

    pub mod dma {
        use serde::{Deserialize, Serialize};

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct File {
            #[serde(default)]
            pub instances: Vec<Instance>,
            #[serde(default)]
            pub interconnect: Vec<Interconnect>,
            #[serde(default, rename = "schema_version")]
            pub schema_version: String,
            #[serde(default)]
            pub version: String,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct Instance {
            #[serde(default)]
            pub channels: Vec<Channel>,
            #[serde(default)]
            pub digital_name: String,
            #[serde(default)]
            pub entity_type: String,
            #[serde(default)]
            pub features: Features,
            #[serde(default, rename = "master_ports")]
            pub master_ports: Vec<MasterPort>,
            #[serde(default)]
            pub name: String,
            #[serde(default)]
            pub peripheral_type: String,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct Channel {
            #[serde(default)]
            pub entity_type: String,
            #[serde(default)]
            pub features: ChannelFeatures,
            #[serde(default)]
            pub name: String,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct ChannelFeatures {
            #[serde(default, rename = "g_addressing")]
            pub g_addressing: GAddressing,
            #[serde(default, rename = "g_fifo_size")]
            pub g_fifo_size: i64,
            #[serde(default, rename = "g_linked_list")]
            pub g_linked_list: bool,
            #[serde(default, rename = "g_per_ctrl")]
            pub g_per_ctrl: bool,
            #[serde(default, rename = "g_transfers")]
            pub g_transfers: GTransfers,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct GAddressing {
            #[serde(default, rename = "block_size")]
            pub block_size: bool,
            #[serde(default, rename = "large_offset")]
            pub large_offset: bool,
            #[serde(default)]
            pub linear: bool,
            #[serde(default, rename = "programmable2D")]
            pub programmable2d: bool,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct GTransfers {
            #[serde(default)]
            pub burst: bool,
            #[serde(default)]
            pub single: bool,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct Features {
            #[serde(default, rename = "g_cid_width")]
            pub g_cid_width: i64,
            #[serde(default, rename = "g_max_cid")]
            pub g_max_cid: i64,
            #[serde(default, rename = "g_max_req_id")]
            pub g_max_req_id: i64,
            #[serde(default, rename = "g_max_trig_id")]
            pub g_max_trig_id: i64,
            #[serde(default, rename = "g_nonsec_optionreg")]
            pub g_nonsec_optionreg: i64,
            #[serde(default, rename = "g_num_channels")]
            pub g_num_channels: i64,
            #[serde(default, rename = "g_num_resync_ffs")]
            pub g_num_resync_ffs: i64,
            #[serde(default, rename = "g_privilege")]
            pub g_privilege: bool,
            #[serde(default, rename = "g_sec_optionreg")]
            pub g_sec_optionreg: i64,
            #[serde(default, rename = "g_trustzone")]
            pub g_trustzone: bool,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct MasterPort {
            #[serde(default, rename = "data_width")]
            pub data_width: i64,
            #[serde(default)]
            pub id: i64,
            #[serde(default)]
            pub name: String,
            #[serde(default, rename = "type")]
            pub type_field: String,
        }

        #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct Interconnect {
            #[serde(default)]
            pub dma_instance: String,
            #[serde(default)]
            pub event: String,
            #[serde(default)]
            pub instance: String,
            #[serde(default, rename = "signal_id")]
            pub signal_id: Option<String>,
            #[serde(default, rename = "type")]
            pub type_field: String,
        }
    }
}

type BuildPins = (
    HashMap<String, xml::Pin>,
    Vec<PackagePin>,
    HashMap<String, Vec<stm32_data_serde::chip::core::peripheral::Pin>>,
    Characteristics,
);

fn build_pins(f: &pinout::File) -> BuildPins {
    let package_pins: Vec<PackagePin> = f
        .bonds
        .iter()
        .filter(|b| pin_matches(&b.die_pad).is_some())
        .filter_map(|b| {
            Some(PackagePin {
                position: b.position.clone(),
                signals: vec![clean_pin(&b.die_pad)?],
            })
        })
        .collect();

    let mut pins: HashMap<String, xml::Pin> = f
        .bonds
        .iter()
        .filter(|b| pin_matches(&b.die_pad).is_some())
        .map(|b| {
            (
                b.die_pad.clone(),
                xml::Pin {
                    name: b.die_pad.clone(),
                    position: b.position.clone(),
                    signals: Vec::new(),
                },
            )
        })
        .collect();

    let mut gpio_af: HashMap<String, Vec<stm32_data_serde::chip::core::peripheral::Pin>> = HashMap::new();

    for signal in &f.signals {
        let Some(entry) = pins.get_mut(&signal.die_pad) else {
            continue;
        };

        entry.signals.push(PinSignal {
            name: signal.name.clone(),
        });

        let af = if let Some(function) = &signal.function.id {
            function.trim_start_matches("AF").parse::<u8>().ok()
        } else {
            None
        };

        gpio_af
            .entry(signal.instance.clone())
            .or_default()
            .push(stm32_data_serde::chip::core::peripheral::Pin {
                pin: signal.die_pad.clone(),
                signal: signal.name.to_string(),
                af: af,
            });
    }

    (pins, package_pins, gpio_af, f.characteristics.clone())
}

type BuildPeripherals = HashMap<String, xml::Ip>;

fn build_peripherals(f: &peripherals::File) -> BuildPeripherals {
    f.peripherals
        .iter()
        .map(|p| {
            (
                p.name.clone(),
                xml::Ip {
                    instance_name: p.name.clone(),
                    name: p.peripheral_type.to_ascii_uppercase(),
                    version: p.digital_name.clone(),
                },
            )
        })
        .collect()
}

fn build_dma(f: &dma::File) -> ChipDma {
    let mut peripherals: HashMap<String, Vec<peripheral::DmaChannel>> = HashMap::with_capacity(f.interconnect.len());

    for instance in &f.instances {
        for interconnect in f
            .interconnect
            .iter()
            .filter(|c| c.dma_instance == instance.name)
            .filter(|c| c.type_field == "request")
        {
            let Some(signal_id) = &interconnect.signal_id else {
                trace!("failed to get signal_id for {}", interconnect.event);
                continue;
            };

            peripherals
                .entry(interconnect.instance.clone())
                .or_default()
                .push(peripheral::DmaChannel {
                    signal: signal_id.clone(),
                    dma: Some(instance.name.clone()),
                    channel: None,
                    dmamux: None,
                    remap: Vec::new(),
                    request: None, // TODO: parse request from RM
                });
        }
    }

    ChipDma {
        peripherals: peripherals,
        channels: f
            .instances
            .iter()
            .map(|instance| {
                instance.channels.iter().enumerate().map(|(i, c)| DmaChannels {
                    name: c.name.clone(),
                    dma: instance.name.clone(),
                    channel: i.try_into().unwrap(),
                    dmamux: None,
                    dmamux_channel: None,
                    supports_2d: None,
                })
            })
            .flatten()
            .collect(),
    }
}

fn build_exti(f: &exti::File) -> HashMap<String, String> {
    f.lines
        .iter()
        .filter_map(|l| Some((l.line_id.clone(), l.interconnect.iter().next()?.instance.clone())))
        .filter(|(_, peri)| !peri.starts_with("GPIO"))
        .collect()
}

fn build_interrupts(f: &interrupts::File, exti_map: &HashMap<String, String>) -> Vec<String> {
    f.instances
        .iter()
        .map(|i| i.interrupts.iter())
        .flatten()
        .map(|irq| {
            // TODO: handle DMA channels
            let mut peripherals = irq
                .peripherals
                .iter()
                .map(|p| exti_map.get(&p.name).unwrap_or(&p.name).clone());

            irq.name.clone() + "_IRQn" + ":Y:" + &peripherals.join(",") + "::"
        })
        .collect()
}

struct PackageDirectory {
    root: PathBuf,
    pinouts: HashMap<PathBuf, BuildPins>,
    peripherals: HashMap<PathBuf, BuildPeripherals>,
    dma: HashMap<PathBuf, ChipDma>,
    exti: HashMap<PathBuf, HashMap<String, String>>,
    interrupts: HashMap<PathBuf, Vec<String>>,
}

impl PackageDirectory {
    pub fn new(path: PathBuf) -> Self {
        Self {
            root: path,
            pinouts: HashMap::new(),
            peripherals: HashMap::new(),
            dma: HashMap::new(),
            exti: HashMap::new(),
            interrupts: HashMap::new(),
        }
    }

    pub fn load(
        &mut self,
        pinouts: &Path,
        peripherals: &Path,
        dma: &Path,
        exti: &Path,
        interrupts: &Path,
    ) -> anyhow::Result<(
        &BuildPins,
        &BuildPeripherals,
        &ChipDma,
        &HashMap<String, String>,
        &Vec<String>,
    )> {
        let load_file = |path: &Path| fs::read_to_string(self.root.join(path));

        let pinouts = entry_or(&mut self.pinouts, pinouts.to_path_buf(), || {
            Ok(build_pins(&serde_json::from_str(&load_file(pinouts)?)?))
        })?;

        let peripherals = entry_or(&mut self.peripherals, peripherals.to_path_buf(), || {
            Ok(build_peripherals(&serde_json::from_str(&load_file(peripherals)?)?))
        })?;

        let dma = entry_or(&mut self.dma, dma.to_path_buf(), || {
            Ok(build_dma(&serde_json::from_str(&load_file(dma)?)?))
        })?;

        let exti = entry_or(&mut self.exti, exti.to_path_buf(), || {
            Ok(build_exti(&serde_json::from_str(&load_file(exti)?)?))
        })?;

        let interrupts = entry_or(&mut self.interrupts, interrupts.to_path_buf(), || {
            Ok(build_interrupts(&serde_json::from_str(&load_file(interrupts)?)?, exti))
        })?;

        Ok((pinouts, peripherals, dma, exti, interrupts))
    }
}

pub fn parse_packages(
    chips: &mut HashMap<String, Chip>,
    chip_groups: &mut Vec<ChipGroup>,
    af: &mut Af,
    dmas: &mut crate::dma::DmaChannels,
    irqs: &mut ChipInterrupts,
) -> anyhow::Result<()> {
    let mut files: Vec<_> = glob::glob("sources/cubeprogdb2/**/*.pdsc")?
        .map(Result::unwrap)
        .collect();
    files.sort();

    for f in files {
        let mut d = PackageDirectory::new(f.parent().unwrap().to_path_buf());

        parse_package(f, &mut d, chips, chip_groups, af, dmas, irqs)?;
    }

    Ok(())
}

fn parse_package(
    f: PathBuf,
    d: &mut PackageDirectory,
    chips: &mut HashMap<String, Chip>,
    chip_groups: &mut Vec<ChipGroup>,
    af: &mut Af,
    dmas: &mut crate::dma::DmaChannels,
    irqs: &mut ChipInterrupts,
) -> anyhow::Result<()> {
    let mut groups: HashMap<String, ChipGroup> = HashMap::new();

    let parsed: Package = quick_xml::de::from_str(&std::fs::read_to_string(f)?)?;
    let package_features = HashSet::from(["SOP", "QFN", "QFP"]);

    for family in parsed.devices.families {
        for subfamily in family.sub_families {
            for device in subfamily.devices {
                let chip_name = &device.name;
                let mut group = groups.entry(chip_name.clone());
                let mut chip = chips.entry(chip_name.clone());
                let mut chip_dma = dmas.0.entry(chip_name.clone());
                let mut af = af.0.entry(chip_name.clone());

                for (_variant, environments, features) in device.variants.iter().map(|variant| {
                    (
                        variant,
                        family
                            .environments
                            .iter()
                            .chain(subfamily.environments.iter())
                            .chain(device.environments.iter())
                            .chain(variant.environments.iter()),
                        family
                            .features
                            .iter()
                            .chain(subfamily.features.iter())
                            .chain(device.features.iter())
                            .chain(variant.features.iter()),
                    )
                }) {
                    let features: Vec<_> = features.collect();
                    let (descriptors, attributes): (Vec<_>, Vec<_>) = environments
                        .map(|e| (&e.device.descriptors.descriptors, &e.device.attributes.attributes))
                        .unzip();

                    let descriptors: HashMap<_, _> = descriptors
                        .iter()
                        .map(|d| d.iter())
                        .flatten()
                        .map(|d| (&*d.schema_type, d))
                        .collect();

                    let attributes: HashMap<_, _> = attributes
                        .iter()
                        .map(|a| a.iter())
                        .flatten()
                        .map(|a| (&*a.name, a))
                        .collect();

                    let Some((
                        ppn,
                        peripherals_descriptor,
                        pinout_descriptor,
                        dma_descriptor,
                        interrupt_descriptor,
                        exti_descriptor,
                        package_feature,
                    )) = (|| {
                        Some((
                            attributes.get("PPN")?,
                            descriptors.get("peripherals")?,
                            descriptors.get("pinout")?,
                            descriptors.get("DMA")?,
                            descriptors.get("NVIC")?,
                            descriptors.get("EXTI")?,
                            features.iter().find(|f| package_features.contains(&*f.feature_type))?,
                        ))
                    })()
                    else {
                        continue;
                    };

                    let ((pins, package_pins, gpio_af, characteristics), peripherals, dma, _exti, interrupts) = d
                        .load(
                            pinout_descriptor.as_path(),
                            peripherals_descriptor.as_path(),
                            dma_descriptor.as_path(),
                            exti_descriptor.as_path(),
                            interrupt_descriptor.as_path(),
                        )?;

                    let group = occupied_entry_or(&mut group, || ChipGroup {
                        chip_names: vec![chip_name.clone()],
                        headers: device.compiles.iter().map(|c| c.define.to_ascii_lowercase()).collect(),
                        cores: family.processors.iter().map(|p| p.core.clone()).collect(),
                        ips: peripherals.clone(),
                        pins: HashMap::new(),
                        family: family.family.clone(),
                        line: subfamily.sub_family.clone(),
                        die: format!("DIE{}", characteristics.die_name),
                        gpio_af: Some(device.name.clone()),
                    });

                    group.ips.entry("NVIC".to_string()).or_insert_with(|| xml::Ip {
                        instance_name: "NVIC".to_string(),
                        name: "NVIC".to_string(),
                        version: chip_name.clone(),
                    });

                    merge_pins(&mut group.pins, pins.clone().into_values());

                    occupied_entry_or(&mut chip, || Chip { packages: Vec::new() })
                        .packages
                        .push(stm32_data_serde::chip::Package {
                            name: ppn.value.clone(),
                            package: package_feature.name.clone(),
                            pins: package_pins.clone(),
                        });

                    occupied_entry_or(&mut chip_dma, || dma.clone());

                    occupied_entry_or(&mut af, || HashMap::new()).extend(gpio_af.clone());

                    irqs.irqs
                        .entry(("NVIC".to_string(), chip_name.clone()))
                        .or_insert_with(|| interrupts.clone());
                }
            }
        }
    }

    chip_groups.extend(groups.into_values());

    Ok(())
}
