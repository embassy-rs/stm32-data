//! Shared inputs for chip generation.
//!
//! Everything in [`GenCtx`] is parsed once in `main` and then read by every
//! chip group. Bundling it into one struct keeps the generator's function
//! signatures from growing by one parameter every time a new input source is
//! added.

use std::collections::{HashMap, HashSet};

use crate::chips::Chip;
use crate::generator::Extra;
use crate::{dma, docs, gpio_af, header, interrupts, low_power, memory, perimap, rcc, trigger};

pub struct GenCtx<'a> {
    /// CMSIS headers, keyed by header file stem.
    pub headers: &'a header::Headers,
    /// GPIO alternate-function tables, keyed by IP version.
    pub af: &'a gpio_af::Af,
    /// `chip:peripheral:ip-version` -> register block.
    pub perimap: &'a perimap::Perimap,
    /// Peripheral -> lowest power mode it keeps working in.
    pub stop_modes: &'a low_power::ChipStopModes,
    /// Peripheral -> trigger signals it emits and consumes.
    pub triggers: &'a trigger::Triggers,
    /// Chip -> flash/SRAM layout.
    pub chip_memories: &'a memory::ChipMemories,
    /// `kind_version` -> the register blocks defined by that YAML file.
    pub blocks: &'a HashMap<String, HashSet<String>>,
    /// Interrupt tables from the CubeDB NVIC files.
    pub chip_interrupts: &'a interrupts::ChipInterrupts,
    /// Parsed `rcc_*` register files: enable/reset bits and clock muxes.
    pub peripheral_to_clock: &'a rcc::ParsedRccs,
    /// DMA IP definitions, keyed by IP version.
    pub dma_channels: &'a dma::DmaChannels,
    /// Package/pin information per chip.
    pub chips: &'a HashMap<String, Chip>,
    /// Reference manuals per chip.
    pub docs: &'a docs::Docs,
    /// Hand-written overlays from `data/extra/*.yaml`.
    pub extras: &'a [Extra],
}
