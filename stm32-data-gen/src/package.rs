use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Ok;
use itertools::Itertools;
use log::trace;
use serde::{Deserialize, Serialize};
use stm32_data_serde::chip::PackagePin;
use stm32_data_serde::chip::core::{DmaChannels, peripheral};

use crate::chips::xml::PinSignal;
use crate::chips::{Chip, ChipGroup, merge_pins, xml};
use crate::dma::{ChipDma, load_dma_mux};
use crate::gpio_af::{self, Af, clean_pin, pin_matches};
use crate::interrupts::{ChipInterrupts, IrqSpec};
use crate::package::schema::pinout::Characteristics;
use crate::package::schema::{dma, exti, interrupts, peripherals, pinout};
use crate::util::{EntryFns, HashMapFns, new_regex_map};

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
    #[serde(default, rename = "flashinfo")]
    pub flashinfos: Vec<Flashinfo>,
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

pub struct BuiltPins {
    pub pins_by_die_pad: HashMap<String, xml::Pin>,
    pub package_pins: Vec<PackagePin>,
    pub gpio_af: HashMap<String, Vec<stm32_data_serde::chip::core::peripheral::Pin>>,
    pub characteristics: Characteristics,
}

/// The parsed artifacts of one package directory, handed back to
/// [`parse_package`].
///
/// `Arc`ed because several chip variants share a directory: each file is
/// parsed once and every caller gets a cheap handle. Callers clone only the
/// fields they need to own — the accumulators take ownership and the data
/// ends up in the generated output.
///
/// Note this deliberately excludes the EXTI map: that is an input to
/// [`build_interrupts`], not an output.
#[derive(Clone)]
struct Loaded {
    pins: Arc<BuiltPins>,
    peripherals: Arc<BuildPeripherals>,
    dma: Arc<ChipDma>,
    interrupts: Arc<Vec<IrqSpec>>,
}

fn build_pins(f: &pinout::File) -> BuiltPins {
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

        let Some((_, short_signal_name)) = gpio_af::parse_signal_name(&signal.name) else {
            continue;
        };
        gpio_af
            .entry(signal.instance.clone())
            .or_default()
            .push(stm32_data_serde::chip::core::peripheral::Pin {
                pin: signal.die_pad.clone(),
                signal: short_signal_name.to_string(),
                af,
            });
    }

    BuiltPins {
        pins_by_die_pad: pins,
        package_pins,
        gpio_af,
        characteristics: f.characteristics.clone(),
    }
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

fn build_dma(f: &dma::File, dma_map: &DmaMap, chip_name: &str) -> ChipDma {
    let mut peripherals: HashMap<String, Vec<peripheral::DmaChannel>> = HashMap::with_capacity(f.interconnect.len());

    for instance in &f.instances {
        let request_map = dma_map.get(&format!("{}:{}:{}", chip_name, instance.name, instance.digital_name));

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

            let signal = if interconnect.instance.starts_with("ADC") && signal_id.eq_ignore_ascii_case("global") {
                interconnect.instance.clone()
            } else {
                signal_id.to_uppercase()
            };

            // DB2 names DAC channel instances DAC1_CH1 / DAC1_CH2, but DMA
            // requests must be associated with the DAC1 peripheral.
            let peripheral_name = match interconnect.instance.rsplit_once('_') {
                Some((name, "CH1" | "CH2")) if name.starts_with("DAC") => name,
                _ => interconnect.instance.as_str(),
            };

            peripherals
                .entry(peripheral_name.to_string())
                .or_default()
                .push(peripheral::DmaChannel {
                    signal,
                    dma: Some(instance.name.clone()),
                    channel: None,
                    dmamux: None,
                    remap: Vec::new(),
                    request: if let Some(request_map) = request_map
                        && let Some(request) = request_map.get(&interconnect.event.to_ascii_uppercase())
                    {
                        Some(*request)
                    } else {
                        None
                    },
                });
        }
    }

    ChipDma {
        peripherals,
        channels: f
            .instances
            .iter()
            .flat_map(|instance| {
                instance.channels.iter().enumerate().map(|(i, c)| DmaChannels {
                    name: c.name.clone(),
                    dma: instance.name.clone(),
                    channel: i.try_into().unwrap(),
                    dmamux: None,
                    dmamux_channel: None,
                    supports_2d: None,
                })
            })
            .collect(),
    }
}

fn build_exti(f: &exti::File) -> HashMap<String, String> {
    f.lines
        .iter()
        .filter_map(|l| Some((l.line_id.clone(), l.interconnect.first()?.instance.clone())))
        .filter(|(_, peri)| !peri.starts_with("GPIO"))
        .collect()
}

fn build_interrupts(f: &interrupts::File, exti_map: &HashMap<String, String>) -> Vec<IrqSpec> {
    f.instances
        .iter()
        .flat_map(|i| i.interrupts.iter())
        .map(|irq| {
            // TODO: handle DMA channels
            let mut peripherals = irq
                .peripherals
                .iter()
                .map(|p| exti_map.get(&p.name).unwrap_or(&p.name).clone());

            let flag = if irq.name.starts_with("EXTI") { "EXTI" } else { "Y" };
            IrqSpec {
                name: irq.name.clone() + "_IRQn",
                flags: vec![flag.to_string()],
                f2: peripherals.join(","),
                f3: String::new(),
                f4: String::new(),
            }
        })
        .collect()
}

struct PackageDirectory {
    root: PathBuf,
    pinouts: HashMap<PathBuf, Arc<BuiltPins>>,
    peripherals: HashMap<PathBuf, Arc<BuildPeripherals>>,
    dma: HashMap<PathBuf, Arc<ChipDma>>,
    exti: HashMap<PathBuf, Arc<HashMap<String, String>>>,
    interrupts: HashMap<PathBuf, Arc<Vec<IrqSpec>>>,
}

/// The five per-package JSON files of a package directory.
pub struct PackageFiles<'a> {
    pub pinouts: &'a Path,
    pub peripherals: &'a Path,
    pub dma: &'a Path,
    pub exti: &'a Path,
    pub interrupts: &'a Path,
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

    pub fn load(&mut self, files: PackageFiles<'_>, dma_map: &DmaMap, chip_name: &str) -> anyhow::Result<Loaded> {
        let PackageFiles {
            pinouts,
            peripherals,
            dma,
            exti,
            interrupts,
        } = files;
        let load_file = |path: &Path| fs::read_to_string(self.root.join(path));

        // Each file is cached under its own path, so a file shared by several
        // variants is still parsed exactly once. Note `dma` is keyed by path
        // alone even though `build_dma` takes `chip_name`; that is how it has
        // always behaved, so do not widen the key without re-checking output.
        let pins = self
            .pinouts
            .get_or_try_insert_with(pinouts.to_path_buf(), || {
                Ok(Arc::new(build_pins(&serde_json::from_str(&load_file(pinouts)?)?)))
            })?
            .clone();

        let peripherals = self
            .peripherals
            .get_or_try_insert_with(peripherals.to_path_buf(), || {
                Ok(Arc::new(build_peripherals(&serde_json::from_str(&load_file(
                    peripherals,
                )?)?)))
            })?
            .clone();

        let dma = self
            .dma
            .get_or_try_insert_with(dma.to_path_buf(), || {
                Ok(Arc::new(build_dma(
                    &serde_json::from_str(&load_file(dma)?)?,
                    dma_map,
                    chip_name,
                )))
            })?
            .clone();

        // Only an input to `build_interrupts`, so not part of `Loaded`.
        let exti = self.exti.get_or_try_insert_with(exti.to_path_buf(), || {
            Ok(Arc::new(build_exti(&serde_json::from_str(&load_file(exti)?)?)))
        })?;

        let interrupts = self
            .interrupts
            .get_or_try_insert_with(interrupts.to_path_buf(), || {
                Ok(Arc::new(build_interrupts(
                    &serde_json::from_str(&load_file(interrupts)?)?,
                    exti,
                )))
            })?
            .clone();

        Ok(Loaded {
            pins,
            peripherals,
            dma,
            interrupts,
        })
    }
}

struct DmaMap {
    map: regex_map::RegexMap<HashMap<String, u8>>,
}

impl DmaMap {
    pub fn new(map: &[(&str, &str)]) -> anyhow::Result<Self> {
        Ok(Self {
            map: new_regex_map(
                map.iter()
                    .map(|(k, v)| (k, load_dma_mux(v).unwrap_or_else(|_| panic!("failed to load: {v}")))),
            ),
        })
    }

    pub fn get(&self, key: &str) -> Option<&HashMap<String, u8>> {
        self.map.get(key).next()
    }
}

pub fn parse_packages(
    chips: &mut HashMap<String, Chip>,
    chip_groups: &mut Vec<ChipGroup>,
    af: &mut Af,
    dmas: &mut crate::dma::DmaChannels,
    irqs: &mut ChipInterrupts,
    filter: &Option<String>,
) -> anyhow::Result<()> {
    let mut files: Vec<_> = glob::glob("sources/cubeprogdb2/**/*.pdsc")?.collect::<Result<_, _>>()?;
    files.sort();
    files.retain(|f| {
        !matches!(
            (filter, f.parent().and_then(|d| d.file_name())),
            (Some(filter), Some(dir_name))
                if !dir_name.to_ascii_lowercase().to_string_lossy().starts_with(filter)
        )
    });

    let dma_map = DmaMap::new(&[
        ("STM32C5.*:LPDMA.*:.*", "C5_LPDMA.yaml"),
        ("STM32G5.*:GPDMA.*:.*", "G5_GPDMA.yaml"),
    ])?;

    // Parsing the `.pdsc` XML and the JSON descriptors it points at dominates
    // this phase; only the merge back into the accumulators has to be ordered,
    // so parse the files in parallel and merge them in file order (the same
    // split `chips::parse_groups` uses).
    let parse_one = |f: &PathBuf| -> anyhow::Result<ParsedPackage> {
        let mut parsed = ParsedPackage::new();
        let mut d = PackageDirectory::new(f.parent().unwrap().to_path_buf());
        let sink = PackageSink {
            chips: &mut parsed.chips,
            chip_groups: &mut parsed.chip_groups,
            af: &mut parsed.af,
            dmas: &mut parsed.dmas,
            irqs: &mut parsed.irqs,
        };
        parse_package(f.clone(), &mut d, sink, &dma_map)?;
        Ok(parsed)
    };

    #[cfg(feature = "rayon")]
    let parsed = {
        use rayon::prelude::*;
        files.par_iter().map(parse_one).collect::<anyhow::Result<Vec<_>>>()?
    };
    #[cfg(not(feature = "rayon"))]
    let parsed = files.iter().map(parse_one).collect::<anyhow::Result<Vec<_>>>()?;

    let mut shared = PackageSink {
        chips,
        chip_groups,
        af,
        dmas,
        irqs: &mut irqs.irqs,
    };
    for parsed in parsed {
        parsed.merge(&mut shared);
    }

    Ok(())
}

/// Everything [`parse_package`] merges one package into.
///
/// Bundling the accumulators keeps the parameter list from growing every time
/// another cross-chip table is added. The same view is used twice: once over
/// one file's fresh [`ParsedPackage`] while that file is parsed, and once over
/// the shared accumulators when the parses are merged back in.
///
/// `irqs` is the raw map rather than a [`ChipInterrupts`] because a per-file
/// parse only ever fills in entries; the interrupt signals table belongs to
/// the shared accumulator alone.
struct PackageSink<'a> {
    chips: &'a mut HashMap<String, Chip>,
    chip_groups: &'a mut Vec<ChipGroup>,
    af: &'a mut Af,
    dmas: &'a mut crate::dma::DmaChannels,
    irqs: &'a mut HashMap<(String, String), Vec<String>>,
}

/// The accumulators one `.pdsc` file produced.
///
/// Each file is parsed into its own bundle of owned data, which is what lets
/// the parses run in parallel; [`ParsedPackage::merge`] folds them back into
/// the shared accumulators in file order.
struct ParsedPackage {
    chips: HashMap<String, Chip>,
    chip_groups: Vec<ChipGroup>,
    af: Af,
    dmas: crate::dma::DmaChannels,
    irqs: HashMap<(String, String), Vec<String>>,
}

impl ParsedPackage {
    fn new() -> Self {
        Self {
            chips: HashMap::new(),
            chip_groups: Vec::new(),
            af: Af(HashMap::new()),
            dmas: crate::dma::DmaChannels(HashMap::new()),
            irqs: HashMap::new(),
        }
    }

    /// Folds this file's results into the shared accumulators, keeping the
    /// semantics the serial loop had: packages and pins are appended to
    /// whatever is already there, while a chip's DMA and IRQ entries are only
    /// filled in when no earlier file provided them.
    fn merge(self, shared: &mut PackageSink<'_>) {
        let ParsedPackage {
            chips,
            chip_groups,
            af,
            dmas,
            irqs,
        } = self;

        for (name, chip) in chips {
            shared
                .chips
                .entry(name)
                .or_insert_with(|| Chip { packages: Vec::new() })
                .packages
                .extend(chip.packages);
        }

        shared.chip_groups.extend(chip_groups);

        for (name, gpio_af) in af.0 {
            shared.af.0.entry(name).or_default().extend(gpio_af);
        }

        for (name, dma) in dmas.0 {
            shared.dmas.0.entry(name).or_insert(dma);
        }

        for (key, irqs) in irqs {
            shared.irqs.entry(key).or_insert(irqs);
        }
    }
}

/// One variant environment's answer to "does this describe a package, and
/// where are its parts": the PPN attribute, the five required descriptors and
/// the package-shape feature, all borrowed from the parsed `.pdsc` data.
struct Resolved<'a> {
    ppn: &'a Attribute,
    peripherals_descriptor: &'a Descriptor,
    pinout_descriptor: &'a Descriptor,
    dma_descriptor: &'a Descriptor,
    interrupt_descriptor: &'a Descriptor,
    exti_descriptor: &'a Descriptor,
    package_feature: &'a Feature,
}

/// Picks the PPN attribute, the five descriptors (`peripherals`, `pinout`,
/// `DMA`, `NVIC`, `EXTI`) and the package feature out of one variant's
/// environment. Returns `None` when any of them is missing, which the caller
/// treats as "this variant does not describe a package" and skips.
fn resolve_descriptors<'a>(
    attributes: &HashMap<&str, &'a Attribute>,
    descriptors: &HashMap<&str, &'a Descriptor>,
    features: &[&'a Feature],
    package_features: &HashSet<&str>,
) -> Option<Resolved<'a>> {
    Some(Resolved {
        ppn: *attributes.get("PPN")?,
        peripherals_descriptor: *descriptors.get("peripherals")?,
        pinout_descriptor: *descriptors.get("pinout")?,
        dma_descriptor: *descriptors.get("DMA")?,
        interrupt_descriptor: *descriptors.get("NVIC")?,
        exti_descriptor: *descriptors.get("EXTI")?,
        package_feature: *features.iter().find(|f| package_features.contains(&*f.feature_type))?,
    })
}

fn parse_package(f: PathBuf, d: &mut PackageDirectory, sink: PackageSink<'_>, dma_map: &DmaMap) -> anyhow::Result<()> {
    let PackageSink {
        chips,
        chip_groups,
        af,
        dmas,
        irqs,
    } = sink;

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
                        .flat_map(|d| d.iter())
                        .map(|d| (&*d.schema_type, d))
                        .collect();

                    let attributes: HashMap<_, _> = attributes
                        .iter()
                        .flat_map(|a| a.iter())
                        .map(|a| (&*a.name, a))
                        .collect();

                    let Some(Resolved {
                        ppn,
                        peripherals_descriptor,
                        pinout_descriptor,
                        dma_descriptor,
                        interrupt_descriptor,
                        exti_descriptor,
                        package_feature,
                    }) = resolve_descriptors(&attributes, &descriptors, &features, &package_features)
                    else {
                        continue;
                    };

                    let loaded = d.load(
                        PackageFiles {
                            pinouts: pinout_descriptor.as_path(),
                            peripherals: peripherals_descriptor.as_path(),
                            dma: dma_descriptor.as_path(),
                            exti: exti_descriptor.as_path(),
                            interrupts: interrupt_descriptor.as_path(),
                        },
                        dma_map,
                        chip_name.as_str(),
                    )?;
                    let BuiltPins {
                        pins_by_die_pad,
                        package_pins,
                        gpio_af,
                        characteristics,
                    } = &*loaded.pins;

                    let group = group.or_insert_with_mut(|| ChipGroup {
                        chip_names: vec![chip_name.clone()],
                        headers: device.compiles.iter().map(|c| c.define.to_ascii_lowercase()).collect(),
                        cores: family.processors.iter().map(|p| p.core.clone()).collect(),
                        ips: loaded.peripherals.as_ref().clone(),
                        pins: HashMap::new(),
                        family: family.family.trim_end_matches("Series").trim().to_string(),
                        line: subfamily.sub_family.clone(),
                        die: format!("DIE{}", characteristics.die_name),
                        gpio_af: Some(device.name.clone()),
                    });

                    group.ips.entry("NVIC".to_string()).or_insert_with(|| xml::Ip {
                        instance_name: "NVIC".to_string(),
                        name: "NVIC".to_string(),
                        version: chip_name.clone(),
                    });

                    merge_pins(&mut group.pins, pins_by_die_pad.values().cloned());

                    chip.or_insert_with_mut(|| Chip { packages: Vec::new() }).packages.push(
                        stm32_data_serde::chip::Package {
                            name: ppn.value.clone(),
                            package: package_feature.name.clone(),
                            pins: package_pins.clone(),
                        },
                    );

                    chip_dma.or_insert_with_mut(|| loaded.dma.as_ref().clone());

                    af.or_insert_with_mut(HashMap::new).extend(gpio_af.clone());

                    irqs
                        .entry(("NVIC".to_string(), chip_name.clone()))
                        .or_insert_with(|| loaded.interrupts.iter().map(ToString::to_string).collect());
                }
            }
        }
    }

    chip_groups.extend(groups.into_values());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c5_dma_map() -> DmaMap {
        let requests = serde_yaml::from_str(include_str!("../../data/dmamux/C5_LPDMA.yaml")).unwrap();
        DmaMap {
            map: new_regex_map([("STM32C5.*:LPDMA.*:.*", requests)]),
        }
    }

    #[test]
    fn dac_dma_channels_belong_to_the_dac_peripheral() {
        let dma_map = c5_dma_map();

        // DB2 describes single- and dual-channel DACs using channel instances.
        for channel_count in [1, 2] {
            let mut file = dma::File::default();
            for controller in ["LPDMA1", "LPDMA2"] {
                file.instances.push(dma::Instance {
                    name: controller.into(),
                    ..Default::default()
                });
                for channel in 1..=channel_count {
                    file.interconnect.push(dma::Interconnect {
                        dma_instance: controller.into(),
                        event: format!("dac1_ch{channel}_dma"),
                        instance: format!("DAC1_CH{channel}"),
                        signal_id: Some(format!("ch{channel}")),
                        type_field: "request".into(),
                    });
                }
            }

            let result = build_dma(&file, &dma_map, "STM32C531RC");
            let requests = result.peripherals.get("DAC1").expect("missing DAC1 DMA requests");
            assert_eq!(result.peripherals.len(), 1);
            assert_eq!(requests.len(), 2 * channel_count);
            for controller in ["LPDMA1", "LPDMA2"] {
                for channel in 1..=channel_count {
                    let request = requests
                        .iter()
                        .find(|r| r.dma.as_deref() == Some(controller) && r.signal == format!("CH{channel}"))
                        .unwrap();
                    assert_eq!(request.request, Some(72 + channel as u8));
                }
            }
        }
    }

    #[test]
    fn dma_peripheral_instances_and_adc_signals_are_preserved() {
        let file: dma::File = serde_json::from_value(serde_json::json!({
            "instances": [{ "name": "LPDMA1" }],
            "interconnect": [
                { "dmaInstance": "LPDMA1", "event": "dac1_ch1_dma", "instance": "DAC1",
                  "signal_id": "ch1", "type": "request" },
                { "dmaInstance": "LPDMA1", "event": "adc1_dma", "instance": "ADC1",
                  "signal_id": "global", "type": "request" },
                { "dmaInstance": "LPDMA1", "event": "spi1_rx_dma", "instance": "SPI1",
                  "signal_id": "rx", "type": "request" }
            ]
        }))
        .unwrap();
        let dma_map = c5_dma_map();
        let result = build_dma(&file, &dma_map, "STM32C531RC");
        for (peripheral, signal, request) in [("DAC1", "CH1", 73), ("ADC1", "ADC1", 0), ("SPI1", "RX", 4)] {
            let requests = &result.peripherals[peripheral];
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].signal, signal);
            assert_eq!(requests[0].request, Some(request));
        }
    }

    #[test]
    fn synthesized_interrupt_strings_keep_the_raw_grammar_format() {
        let file: interrupts::File = serde_json::from_value(serde_json::json!({
            "instances": [{
                "interrupts": [
                    { "name": "TIM1", "instances": [{ "name": "TIM1" }] },
                    { "name": "EXTI0", "instances": [{ "name": "GPIOA" }] },
                    { "name": "SPI1", "instances": [{ "name": "SPI1" }, { "name": "I2S1" }] }
                ]
            }]
        }))
        .unwrap();

        // These are the exact bytes the old `name + "_IRQn" + t + peris + "::"`
        // concatenation produced, and what `irqs` stores after Display conversion.
        let rendered: Vec<String> = build_interrupts(&file, &HashMap::new())
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            rendered,
            [
                "TIM1_IRQn:Y:TIM1::",
                "EXTI0_IRQn:EXTI:GPIOA::",
                "SPI1_IRQn:Y:SPI1,I2S1::"
            ]
        );
    }
}
