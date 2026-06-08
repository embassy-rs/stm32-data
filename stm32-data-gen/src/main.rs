use clap::{Parser, ValueEnum};
use env_logger::Env;
use log::LevelFilter;

mod check;
mod chips;
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

#[macro_export]
macro_rules! regex {
    ($re:literal) => {{
        ::ref_thread_local::ref_thread_local! {
            static managed REGEX: ::regex::Regex = ::regex::Regex::new($re).unwrap();
        }
        <REGEX as ::ref_thread_local::RefThreadLocal<::regex::Regex>>::borrow(&REGEX)
    }};
}

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

impl Into<LevelFilter> for LogLevel {
    fn into(self) -> LevelFilter {
        match self {
            Self::Off => LevelFilter::Off,
            Self::Error => LevelFilter::Error,
            Self::Warn => LevelFilter::Warn,
            Self::Info => LevelFilter::Info,
            Self::Debug => LevelFilter::Debug,
            Self::Trace => LevelFilter::Trace,
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

    // TODO: apply filter to other groups

    let mut stopwatch = Stopwatch::new();

    let triggers = trigger::Triggers::new();

    stopwatch.section("Parsing headers");
    let headers = header::Headers::parse()?;

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
    let mut dma_channels = dma::DmaChannels::parse()?;

    // stopwatch.section("Parsing GPIO AF");
    let mut af = gpio_af::Af::parse()?;

    stopwatch.section("Parsing chip groups");
    let (mut chips, mut chip_groups) = chips::parse_groups(&args.filter)?;

    stopwatch.section("Parsing packages");

    package::parse_packages(
        &mut chips,
        &mut chip_groups,
        &mut af,
        &mut dma_channels,
        &mut chip_interrupts,
        &args.filter,
    )?;

    stopwatch.section("Processing chips");
    generator::dump_all_chips(
        chip_groups,
        headers,
        af,
        triggers,
        registers.blocks,
        chip_interrupts,
        peripheral_to_clock,
        dma_channels,
        chips,
        docs,
    )?;

    stopwatch.stop();

    Ok(())
}
