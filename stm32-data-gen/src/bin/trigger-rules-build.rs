use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Trigger {
    signal: String,
    source: String,
}

#[derive(Debug, Deserialize)]
struct TriggerRule {
    #[serde(rename = "match")]
    match_: String,
    triggers: Vec<Trigger>,
}

const RAW_DIR: &str = "data/triggers/raw";

fn main() {
    let manifest_path = Path::new(RAW_DIR).join("manifest.txt");
    let manifest =
        std::fs::read_to_string(&manifest_path).unwrap_or_else(|e| panic!("failed to read {manifest_path:?}: {e}"));

    let mut rules: Vec<TriggerRule> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for line in manifest.lines() {
        let file = line.trim();
        if file.is_empty() || file.starts_with('#') {
            continue;
        }
        assert!(seen.insert(file.to_string()), "manifest lists {file} twice");
        let path = Path::new(RAW_DIR).join(file);
        let data = std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read {path:?}: {e}"));
        let mut fragment: Vec<TriggerRule> =
            serde_yaml::from_slice(&data).unwrap_or_else(|e| panic!("failed to parse {path:?}: {e}"));
        rules.append(&mut fragment);
    }

    // Ensure every yaml file in raw/ is listed in the manifest.
    for entry in std::fs::read_dir(RAW_DIR).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        assert!(seen.contains(&file), "{file} is not listed in manifest.txt");
    }

    let mut out = String::new();
    out.push_str("# Peripheral trigger rules, keyed by regex over \"MCU:PERIPHERAL\".\n");
    out.push_str("#\n");
    out.push_str("# GENERATED FILE - do not edit by hand. Regenerate with:\n");
    out.push_str("#     cargo run --release --bin trigger-rules-build\n");
    out.push_str("#\n");
    out.push_str("# Source data lives in data/triggers/raw/, concatenated in the order given by\n");
    out.push_str("# data/triggers/raw/manifest.txt. The first matching entry wins, so order matters.\n");
    out.push_str("# Raw fragments record their provenance (SVD file / RM document and tables) in\n");
    out.push_str("# their header comments.\n\n");

    for rule in &rules {
        out.push_str(&format!("- match: '{}'\n", rule.match_));
        if rule.triggers.is_empty() {
            out.push_str("  triggers: []\n");
        } else {
            out.push_str("  triggers:\n");
            for t in &rule.triggers {
                out.push_str(&format!("    - signal: {}\n", t.signal));
                out.push_str(&format!("      source: {}\n", t.source));
            }
        }
        out.push('\n');
    }

    std::fs::write("data/triggers/rules.yaml", out).unwrap();
    println!("wrote data/triggers/rules.yaml: {} rules", rules.len());
}
