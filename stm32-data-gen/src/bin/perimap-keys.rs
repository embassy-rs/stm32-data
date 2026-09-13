use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use clap::Parser;
use glob::glob;
use serde::Deserialize;

const C5_PERIPHERALS_DIR: &str = "sources/cubeprogdb2/stm32c5/Descriptors/peripherals";

#[derive(Debug, Deserialize)]
struct PeripheralFile {
    #[serde(default)]
    peripherals: Vec<Peripheral>,
}

#[derive(Debug, Deserialize)]
struct Peripheral {
    #[serde(default, rename = "digitalName")]
    digital_name: String,

    #[serde(default)]
    name: String,

    #[serde(default, rename = "peripheralType")]
    peripheral_type: String,
}

impl fmt::Display for Peripheral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.name,
            self.peripheral_type.to_ascii_uppercase(),
            self.digital_name,
        )
    }
}

#[derive(Debug, Deserialize)]
struct McuFile {
    #[serde(default, rename = "IP")]
    ips: Vec<Ip>,
}

#[derive(Debug, Deserialize)]
struct Ip {
    #[serde(rename = "@InstanceName")]
    instance_name: String,

    #[serde(rename = "@Name")]
    name: String,

    #[serde(rename = "@Version")]
    version: String,
}

#[derive(Parser)]
#[command(name = "perimap-keys")]
#[command(about = "List raw peripheral metadata used to build perimap keys")]
struct Args {
    /// Chip name or family prefix, for example STM32C531 or STM32C0
    #[arg(short, long)]
    chip: String,

    /// Peripheral instance or IP name, for example RTC or ADC
    #[arg(short, long)]
    peripheral: String,
}

fn c5_peripheral_files(chip: &str) -> &'static [&'static str] {
    if chip.starts_with("STM32C531") || chip.starts_with("STM32C532") || chip.starts_with("STM32C542") {
        &["D44F_peripherals.json"]
    } else if chip.starts_with("STM32C551") || chip.starts_with("STM32C552") || chip.starts_with("STM32C562") {
        &["D44E_peripherals.json"]
    } else if chip.starts_with("STM32C591") || chip.starts_with("STM32C593") || chip.starts_with("STM32C5A3") {
        &["D45A_peripherals.json"]
    } else if chip == "STM32C5" {
        &[
            "D44E_peripherals.json",
            "D44F_peripherals.json",
            "D45A_peripherals.json",
        ]
    } else {
        &[]
    }
}

fn search_cubeprogdb2(chip: &str, peripheral_name: &str) -> anyhow::Result<BTreeSet<String>> {
    let files = c5_peripheral_files(chip);
    anyhow::ensure!(!files.is_empty(), "unsupported STM32C5 chip or prefix: {chip}");

    let mut results = BTreeSet::new();

    for file_name in files {
        let path = Path::new(C5_PERIPHERALS_DIR).join(file_name);
        let input = std::fs::File::open(&path)?;
        let file: PeripheralFile = serde_json::from_reader(input)?;

        for peripheral in file.peripherals {
            if peripheral.name.eq_ignore_ascii_case(peripheral_name)
                || peripheral.peripheral_type.eq_ignore_ascii_case(peripheral_name)
            {
                results.insert(peripheral.to_string());
            }
        }
    }

    Ok(results)
}

fn search_cubedb(chip: &str, peripheral_name: &str) -> anyhow::Result<BTreeSet<String>> {
    let pattern = format!("sources/cubedb/mcu/{chip}*.xml");
    let mut results = BTreeSet::new();

    for path in glob(&pattern)? {
        let path = path?;
        let content = std::fs::read_to_string(path)?;
        let mcu: McuFile = quick_xml::de::from_str(&content)?;

        for ip in mcu.ips {
            if ip.name.eq_ignore_ascii_case(peripheral_name) || ip.instance_name.eq_ignore_ascii_case(peripheral_name) {
                let version = ip.version.strip_suffix("_Cube").unwrap_or(&ip.version);
                results.insert(format!("{}:{}:{version}", ip.instance_name, ip.name));
            }
        }
    }

    Ok(results)
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let chip = args.chip.to_ascii_uppercase();

    let results = if chip.starts_with("STM32C5") {
        search_cubeprogdb2(&chip, &args.peripheral)?
    } else {
        search_cubedb(&chip, &args.peripheral)?
    };

    anyhow::ensure!(
        !results.is_empty(),
        "no {} metadata found for {}",
        args.peripheral,
        args.chip,
    );

    for key in results {
        println!("{chip}:{key}");
    }

    Ok(())
}
