use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use clap::Parser;
use serde_json;
use stm32_data_serde::Chip;

fn extract_peripheral_kind_from_name(peripheral_name: &str) -> String {
    let name_lower = peripheral_name.to_lowercase();

    // Special cases for peripherals with numbers in their standard names
    if name_lower.starts_with("uart") {
        return "USART".to_string();
    }
    if name_lower.starts_with("i2c") {
        return "I2C".to_string();
    }
    if name_lower.starts_with("i3c") {
        return "I3C".to_string();
    }
    if name_lower.starts_with("dma2d") {
        return "DMA2D".to_string();
    }

    // Default heuristic: take alphabetic prefix
    let kind = name_lower
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect::<String>()
        .to_uppercase();

    kind
}

#[derive(Parser)]
#[command(name = "peripheral-summary")]
#[command(about = "Summarizes peripheral versions by STM32 family")]
struct Args {
    /// Peripheral kind to analyze (e.g., 'adc', 'timer', 'usart'). If not specified, shows all peripherals
    #[arg(short = 'p', long = "peripheral")]
    peripheral: Option<String>,

    /// Directory containing chip JSON files
    #[arg(short = 'd', long = "chips-dir", default_value = "build/data/chips")]
    chips_dir: String,
}

fn main() {
    let args = Args::parse();

    if let Some(peripheral) = &args.peripheral {
        eprintln!("Analyzing peripheral kind: {}", peripheral);
    } else {
        eprintln!("Analyzing all peripheral kinds");
    }
    eprintln!("Reading chip files from: {}", args.chips_dir);
    eprintln!();

    // Map: peripheral_kind -> family -> (versions, has_unsupported)
    let mut peripheral_data: BTreeMap<String, BTreeMap<String, (BTreeSet<String>, bool)>> = BTreeMap::new();
    let mut chip_count = 0;
    let mut processed_count = 0;

    // Read all JSON files in the chips directory
    let chips_path = Path::new(&args.chips_dir);
    if !chips_path.exists() {
        eprintln!("Error: Directory {} does not exist", args.chips_dir);
        std::process::exit(1);
    }

    let entries = match fs::read_dir(chips_path) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Error reading directory {}: {}", args.chips_dir, e);
            std::process::exit(1);
        }
    };

    for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }

        chip_count += 1;

        // Read and parse the JSON file
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("Warning: Failed to read file {:?}: {}", path, e);
                continue;
            }
        };

        let chip: Chip = match serde_json::from_str(&content) {
            Ok(chip) => chip,
            Err(e) => {
                eprintln!("Warning: Failed to parse JSON in {:?}: {}", path, e);
                continue;
            }
        };

        processed_count += 1;

        // Look for peripherals of the specified kind in all cores
        for core in &chip.cores {
            for peripheral in &core.peripherals {
                // First, determine the peripheral kind (always uppercase)
                let peripheral_kind = if let Some(registers) = &peripheral.registers {
                    // Supported peripheral - use registers.kind
                    registers.kind.to_uppercase()
                } else {
                    // Unsupported peripheral - extract kind from name using heuristic
                    extract_peripheral_kind_from_name(&peripheral.name)
                };

                // Skip if we couldn't determine a kind
                if peripheral_kind.is_empty() {
                    eprintln!(
                        "Error: Could not determine peripheral kind for '{}' in chip '{}'",
                        peripheral.name, chip.name
                    );
                    eprintln!("This should never happen - please check the peripheral name extraction logic");
                    std::process::exit(1);
                }

                // Second, check if we should include this peripheral
                let should_include = match &args.peripheral {
                    Some(filter) => peripheral_kind == filter.to_uppercase(),
                    None => true, // Include all when no filter specified
                };

                if should_include {
                    if let Some(registers) = &peripheral.registers {
                        // Supported peripheral with registers
                        let family_data = peripheral_data
                            .entry(peripheral_kind.clone())
                            .or_insert_with(BTreeMap::new);
                        let entry = family_data
                            .entry(chip.family.clone())
                            .or_insert_with(|| (BTreeSet::new(), false));
                        entry.0.insert(registers.version.clone());
                    } else {
                        // Unsupported peripheral (no registers field)
                        let family_data = peripheral_data
                            .entry(peripheral_kind.clone())
                            .or_insert_with(BTreeMap::new);
                        let entry = family_data
                            .entry(chip.family.clone())
                            .or_insert_with(|| (BTreeSet::new(), false));
                        entry.1 = true;
                    }
                }
            }
        }
    }

    eprintln!("Processed {} out of {} chip files", processed_count, chip_count);
    eprintln!();

    if peripheral_data.is_empty() {
        if let Some(peripheral) = &args.peripheral {
            eprintln!("No peripherals of kind '{}' found in any chip", peripheral);
        } else {
            eprintln!("No peripherals found in any chip");
        }
        return;
    }

    // Always generate markdown output
    generate_markdown_output(&peripheral_data, &args.peripheral);
}

fn generate_markdown_output(
    peripheral_data: &BTreeMap<String, BTreeMap<String, (BTreeSet<String>, bool)>>,
    filter_peripheral: &Option<String>,
) {
    // Filter data if a specific peripheral is requested
    let filtered_data = if let Some(peripheral) = filter_peripheral {
        let peripheral_key = peripheral.to_uppercase();
        if let Some(family_data) = peripheral_data.get(&peripheral_key) {
            let mut filtered = BTreeMap::new();
            filtered.insert(peripheral_key, family_data.clone());
            filtered
        } else {
            BTreeMap::new()
        }
    } else {
        peripheral_data.clone()
    };

    if filtered_data.is_empty() {
        if let Some(peripheral) = filter_peripheral {
            println!("No peripherals of kind '{}' found in any chip", peripheral);
        } else {
            println!("No peripherals found in any chip");
        }
        return;
    }

    // Generate the markdown table
    generate_markdown_table(&filtered_data);

    // Generate detailed sections for each peripheral
    generate_markdown_sections(&filtered_data);
}

fn generate_markdown_table(peripheral_data: &BTreeMap<String, BTreeMap<String, (BTreeSet<String>, bool)>>) {
    // Get all families across all peripherals
    let mut all_families: BTreeSet<String> = BTreeSet::new();
    for family_data in peripheral_data.values() {
        all_families.extend(family_data.keys().cloned());
    }

    if all_families.is_empty() {
        println!("No families found with any peripherals");
        return;
    }

    // Generate markdown table
    println!("## Peripheral support by family\n");

    // Header row
    print!("| Peripheral |");
    for family in &all_families {
        let trimmed_family = family.strip_prefix("STM32").unwrap_or(family);
        print!(" {} |", trimmed_family);
    }
    println!();

    // Separator row
    print!("|------------|");
    for _ in &all_families {
        print!("--------|");
    }
    println!();

    // Data rows - one per peripheral with links to sections
    for (peripheral_kind, family_data) in peripheral_data {
        print!("| [{}](#{}) |", peripheral_kind, peripheral_kind.to_lowercase());
        for family in &all_families {
            if let Some((versions, has_unsupported)) = family_data.get(family) {
                let mut cell_content = Vec::new();

                // Add versions if any
                if !versions.is_empty() {
                    let versions_str: Vec<_> = versions.iter().cloned().collect();
                    cell_content.push(versions_str.join(", "));
                }

                // Add red X if has unsupported
                if *has_unsupported {
                    cell_content.push("❌".to_string());
                }

                let content = if cell_content.is_empty() {
                    "".to_string()
                } else {
                    cell_content.join(", ")
                };

                print!(" {} |", content);
            } else {
                print!(" |");
            }
        }
        println!();
    }
    println!();
}

fn generate_markdown_sections(peripheral_data: &BTreeMap<String, BTreeMap<String, (BTreeSet<String>, bool)>>) {
    println!("## Detailed peripheral information\n");

    for (peripheral_kind, family_data) in peripheral_data {
        // Create section header with anchor
        println!("### {}\n", peripheral_kind);

        // Generate the detailed information using the existing logic
        generate_peripheral_detail(peripheral_kind, family_data);
        println!();
    }
}

fn generate_peripheral_detail(
    _peripheral_kind: &str,
    family_peripheral_data: &BTreeMap<String, (BTreeSet<String>, bool)>,
) {
    // Build version_map from family_peripheral_data
    let mut version_map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for (family, (versions, _has_unsupported)) in family_peripheral_data {
        for version in versions {
            version_map
                .entry(version.clone())
                .or_insert_with(BTreeSet::new)
                .insert(family.clone());
        }
    }

    // Also collect families with unsupported peripherals
    let mut unsupported_families: BTreeSet<String> = BTreeSet::new();
    for (family, (_versions, has_unsupported)) in family_peripheral_data {
        if *has_unsupported {
            unsupported_families.insert(family.clone());
        }
    }

    if version_map.is_empty() && unsupported_families.is_empty() {
        println!("No supported or unsupported peripherals found for this peripheral.");
        return;
    }

    println!("**Versions by family:**");
    println!();
    for (version, families) in &version_map {
        println!(
            "- **{}**: {}",
            version,
            families.iter().cloned().collect::<Vec<_>>().join(", ")
        );
    }

    // Add unsupported peripherals if any
    if !unsupported_families.is_empty() {
        println!(
            "- **❌ Unsupported**: {}",
            unsupported_families.iter().cloned().collect::<Vec<_>>().join(", ")
        );
    }
}
