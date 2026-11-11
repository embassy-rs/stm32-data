use std::fs;

use clap::{Parser, ValueEnum};
use env_logger::Env;
use log::{LevelFilter, info};

use crate::ctx::GenCtx;

mod check;
mod chips;
mod config;
mod ctx;
mod dma;
mod docs;
mod generator;
mod gpio_af;
mod header;
mod interrupts;
mod low_power;
mod memory;
mod normalize_peris;
mod package;
mod perimap;
mod rcc;
mod registers;
mod trigger;
mod util;

struct Stopwatch {
    start: std::time::Instant,
    section_start: Option<std::time::Instant>,
}

impl Stopwatch {
    fn new() -> Self {
        eprintln!("Starting timer");
        let start = std::time::Instant::now();
        Self {
            start,
            section_start: None,
        }
    }

    fn section(&mut self, status: &str) {
        let now = std::time::Instant::now();
        self.print_done(now);
        eprintln!("  {status}");
        self.section_start = Some(now);
    }

    fn stop(self) {
        let now = std::time::Instant::now();
        self.print_done(now);
        let total_elapsed = now - self.start;
        eprintln!("Total time: {:.2} seconds", total_elapsed.as_secs_f32());
    }

    fn print_done(&self, now: std::time::Instant) {
        if let Some(section_start) = self.section_start {
            let elapsed = now - section_start;
            eprintln!("    done in {:.2} seconds", elapsed.as_secs_f32());
        }
    }
}

#[derive(Debug, Copy, Clone, Default, ValueEnum)]
pub enum LogLevel {
    /// A level lower than all log levels.
    Off,
    /// Corresponds to the `Error` log level.
    Error,
    /// Corresponds to the `Warn` log level.
    #[default]
    Warn,
    /// Corresponds to the `Info` log level.
    Info,
    /// Corresponds to the `Debug` log level.
    Debug,
    /// Corresponds to the `Trace` log level.
    Trace,
}

impl From<LogLevel> for LevelFilter {
    fn from(val: LogLevel) -> Self {
        match val {
            LogLevel::Off => LevelFilter::Off,
            LogLevel::Error => LevelFilter::Error,
            LogLevel::Warn => LevelFilter::Warn,
            LogLevel::Info => LevelFilter::Info,
            LogLevel::Debug => LevelFilter::Debug,
            LogLevel::Trace => LevelFilter::Trace,
        }
    }
}

/// Generate chip JSON files
#[derive(Parser)]
struct Cli {
    #[arg(long, value_enum, default_value = "warn")]
    /// The log level
    log_level: LogLevel,

    #[arg(long)]
    /// A filter to use to only generate certain chips
    filter: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    pretty_env_logger::formatted_builder()
        .parse_env(Env::default().default_filter_or(match args.log_level {
            LogLevel::Off => "off",
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        }))
        .init();

    let filter = args
        .filter
        .map(|f| f.to_ascii_lowercase())
        .map(|f| f[..f.len().min(7)].to_string());

    if let Some(filter) = &filter {
        info!("using filter: {}", filter);
    }

    let mut stopwatch = Stopwatch::new();

    let perimap = perimap::Perimap::new();
    let stop_modes = low_power::ChipStopModes::new();
    let triggers = trigger::Triggers::new();
    let chip_memories = memory::ChipMemories::new();

    stopwatch.section("Removing build directory");

    let _ = fs::remove_dir_all("build/data");

    stopwatch.section("Parsing headers");
    let headers = header::Headers::parse(&filter)?;

    stopwatch.section("Parsing other stuff");

    // stopwatch.section("Parsing registers");
    let registers = registers::Registers::parse()?;
    registers.write()?;

    // stopwatch.section("Parsing interrupts");
    let mut chip_interrupts = interrupts::ChipInterrupts::parse()?;

    // stopwatch.section("Parsing RCC registers");
    let peripheral_to_clock = rcc::ParsedRccs::parse(&registers)?;

    // stopwatch.section("Parsing docs");
    let docs = docs::Docs::parse()?;

    // stopwatch.section("Parsing DMA");
    let mut dma_channels = dma::DmaChannels::parse(&perimap)?;

    // stopwatch.section("Parsing GPIO AF");
    let mut af = gpio_af::Af::parse()?;

    // stopwatch.section("Parsing extras");
    let extras = generator::load_extras();

    stopwatch.section("Parsing chip groups");
    let (mut chips, mut chip_groups) = chips::parse_groups(&filter)?;

    stopwatch.section("Parsing packages");

    package::parse_packages(
        &mut chips,
        &mut chip_groups,
        &mut af,
        &mut dma_channels,
        &mut chip_interrupts,
        &filter,
    )?;

    stopwatch.section("Processing chips");
    generator::dump_all_chips(
        chip_groups,
        GenCtx {
            headers: &headers,
            af: &af,
            perimap: &perimap,
            stop_modes: &stop_modes,
            triggers: &triggers,
            chip_memories: &chip_memories,
            blocks: &registers.blocks,
            chip_interrupts: &chip_interrupts,
            peripheral_to_clock: &peripheral_to_clock,
            dma_channels: &dma_channels,
            chips: &chips,
            docs: &docs,
            extras: &extras,
        },
    )?;

    stopwatch.stop();

    Ok(())
}
