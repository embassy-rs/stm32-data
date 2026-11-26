//! Runtime configuration read from `data/*.yaml`.
//!
//! These files hold pure data tables that used to be hardcoded in the source.
//! Keeping them in `data/` means a wrong mapping can be fixed without a rebuild
//! (and without a rebuild of the generated output being gated on it), and it
//! keeps the Rust code about *how* to generate chip data rather than *what* the
//! chips are.
//!
//! The tables are process-wide and immutable, so they are loaded lazily on first
//! use and shared by every worker thread.

use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use regex::{Regex, RegexSet};
use serde::Deserialize;
use stm32_data_serde::chip::core::peripheral::rcc::{LowPower, StopModes};
use stm32_data_serde::chip::power::{Dialect, ModeKind};

use crate::util::{new_regex_map, new_regex_set};

/// Parse a YAML config file, panicking with the file path if anything is wrong.
///
/// A codegen tool with a malformed config file has no useful output to produce,
/// so failing loudly with the exact path is the right behaviour.
pub fn load_yaml<T: serde::de::DeserializeOwned>(path: impl AsRef<Path>) -> T {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {path:?}: {e}"));
    serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("failed to parse {path:?}: {e}"))
}

/// `data/ignore.yaml`: chip name patterns excluded from generation.
pub static IGNORE: LazyLock<RegexSet> = LazyLock::new(|| new_regex_set(load_yaml::<Vec<String>>("data/ignore.yaml")));

/// `data/peripheral_bases.yaml`: alternate C header defines that may hold a
/// peripheral's base address.
pub static PERIPHERAL_BASES: LazyLock<HashMap<String, Vec<String>>> =
    LazyLock::new(|| load_yaml("data/peripheral_bases.yaml"));

/// `data/peripherals.yaml`: peripheral naming and presence rules.
pub static PERIPHERALS: LazyLock<PeripheralRules> = LazyLock::new(|| load_yaml("data/peripherals.yaml"));

/// `data/afio.yaml`: STM32F1 AFIO pin-remap tables.
pub static AFIO: LazyLock<AfioTables> = LazyLock::new(|| AfioFile::load().into_tables());

#[derive(Debug, Deserialize)]
pub struct PeripheralRules {
    /// CubeDB instance name -> canonical peripheral name.
    pub normalize: HashMap<String, String>,
    /// Peripherals that are never emitted, even though CubeDB lists them.
    pub fake: Vec<String>,
    /// Peripherals emitted even when CubeDB does not list them, if the C header
    /// defines a base address for them.
    pub ghost: Vec<String>,
    /// Caps on DMA channel count, keyed by `chip:DMA-name`.
    pub dma_channel_counts: HashMap<String, usize>,
}

impl PeripheralRules {
    /// The DMA channel caps as a regex map keyed by `chip:DMA-name`.
    fn dma_channel_counts(&self) -> regex_map::RegexMap<usize> {
        new_regex_map(
            self.dma_channel_counts
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect::<std::collections::BTreeMap<_, _>>(),
        )
    }
}

/// `data/peripherals.yaml` DMA channel caps, keyed by `chip:DMA-name`.
pub static DMA_CHANNEL_COUNTS: LazyLock<regex_map::RegexMap<usize>> =
    LazyLock::new(|| PERIPHERALS.dma_channel_counts());

/// One `AFIO->MAPR`/`MAPR2` remap value and the pins it selects.
#[derive(Debug, Clone, Deserialize)]
pub struct AfioValue {
    pub value: u8,
    pub pins: Vec<String>,
}

/// Remap tables for one set of parts.
pub type AfioEntries = HashMap<String, Vec<AfioValue>>;

/// STM32F1 AFIO pin-remap tables.
///
/// `mapr` is consulted first; a peripheral with no `mapr` entry falls back to
/// `mapr2`, using the first group whose `chips` regex matches the chip name.
pub struct AfioTables {
    pub mapr: AfioEntries,
    pub mapr2: Vec<(Regex, AfioEntries)>,
}

#[derive(Deserialize)]
struct AfioFile {
    mapr: AfioEntries,
    mapr2: Vec<AfioGroup>,
}

#[derive(Deserialize)]
struct AfioGroup {
    /// Regex matched against the chip name.
    chips: String,
    entries: AfioEntries,
}

impl AfioFile {
    fn load() -> Self {
        load_yaml("data/afio.yaml")
    }

    fn into_tables(self) -> AfioTables {
        let mapr2 = self
            .mapr2
            .into_iter()
            .map(|g| {
                let re = Regex::new(&format!("^(?:{})$", g.chips))
                    .unwrap_or_else(|e| panic!("bad `chips` regex {:?} in data/afio.yaml: {e}", g.chips));
                (re, g.entries)
            })
            .collect();
        AfioTables { mapr: self.mapr, mapr2 }
    }
}

/// A `chip:peripheral:ip-version` key -> register block mapping rule.
#[derive(Debug, Clone, Deserialize)]
pub struct PerimapEntry {
    /// Full-string regex matched against `{chip}:{peripheral}:{ip_version}`.
    #[serde(rename = "match")]
    pub pattern: String,
    pub kind: String,
    pub version: String,
    pub block: String,
}

/// `data/perimap.yaml`, in priority order (first match wins).
pub static PERIMAP: LazyLock<Vec<PerimapEntry>> = LazyLock::new(|| load_yaml("data/perimap.yaml"));

/// One chip-level power mode descriptor from `data/low_power.yaml`.
#[derive(Debug, Clone, Deserialize)]
pub struct ChipPowerSpec {
    pub dialect: Dialect,
    #[serde(default)]
    pub modes: Vec<ChipPowerModeSpec>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChipPowerModeSpec {
    pub kind: ModeKind,
    pub selector: u8,
    #[serde(default)]
    pub min_svos: Option<u8>,
    #[serde(default)]
    pub surviving_clocks: Vec<String>,
    #[serde(default)]
    pub min_residency_us: u32,
    #[serde(default)]
    pub exit_latency_us: u32,
}

/// One chip power rule from `data/low_power.yaml`.
#[derive(Debug, Clone, Deserialize)]
pub struct ChipPowerRule {
    /// Full-string regex matched against the chip name.
    #[serde(rename = "match")]
    pub pattern: String,
    pub power: Option<ChipPowerSpec>,
}

/// One peripheral low-power capability rule from `data/low_power.yaml`.
#[derive(Debug, Clone, Deserialize)]
pub struct PeripheralLowPowerRule {
    /// Full-string regex matched against `{mcu}:{peripheral}`.
    #[serde(rename = "match")]
    pub pattern: String,
    #[serde(default)]
    pub modes: Vec<ModeKind>,
    #[serde(default)]
    pub autonomous: bool,
    #[serde(default)]
    pub wake_capable: bool,
    #[serde(default)]
    pub wake_exti: Option<u8>,
}

impl PeripheralLowPowerRule {
    pub fn to_low_power(&self) -> LowPower {
        let mut modes = StopModes::default();
        for kind in &self.modes {
            match kind {
                ModeKind::Stop0 => modes.stop0 = true,
                ModeKind::Stop1 => modes.stop1 = true,
                ModeKind::Stop2 => modes.stop2 = true,
                ModeKind::Stop3 => modes.stop3 = true,
            }
        }
        LowPower {
            modes,
            autonomous: self.autonomous,
            wake_capable: self.wake_capable,
            wake_exti: self.wake_exti,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LowPowerFile {
    pub chips: Vec<ChipPowerRule>,
    pub peripherals: Vec<PeripheralLowPowerRule>,
}

/// `data/low_power.yaml`, in priority order (first match wins per section).
pub static LOW_POWER: LazyLock<LowPowerFile> = LazyLock::new(|| load_yaml("data/low_power.yaml"));
