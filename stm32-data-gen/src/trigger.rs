use std::collections::{HashMap, HashSet};

use regex::Regex;
use serde::Deserialize;

use crate::util::new_regex_map;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Trigger {
    pub signal: String,
    pub source: String,
}

#[derive(Debug, Deserialize)]
struct TriggerRule {
    /// Regex matched against "mcu_name:peripheral".
    #[serde(rename = "match")]
    match_: String,
    triggers: Vec<Trigger>,
}

pub struct Triggers {
    map: regex_map::RegexMap<Vec<Trigger>>,
}

impl Triggers {
    pub fn new() -> Self {
        // Regexmap where the key is mcu_name:peripheral and the value is the trigger list.
        // The first matching rule wins, so the order of rules in the YAML file matters.
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../data/triggers/rules.yaml");
        let data = std::fs::read(path).unwrap();
        let rules: Vec<TriggerRule> =
            serde_yaml::from_slice(&data).unwrap_or_else(|e| panic!("failed to parse data/triggers/rules.yaml: {e}"));

        let trigger_expr = Regex::new(r"(?m)(.+?)(\d+)").unwrap();

        for rule in &rules {
            let mut trigger_sets: HashMap<String, HashSet<&str>> = HashMap::new();

            for trigger in &rule.triggers {
                let matches = trigger_expr.captures(&trigger.signal).unwrap();
                let trigger_set = trigger_sets.entry((&matches[1]).to_string()).or_insert(HashSet::new());

                if !trigger_set.insert(&trigger.source) || trigger.source != trigger.source.to_uppercase() {
                    panic!(
                        "trigger: failed to validate rules for expr {} (source: {})",
                        rule.match_, trigger.source
                    );
                }
            }
        }

        Self {
            map: new_regex_map(rules.iter().map(|rule| (&rule.match_, rule.triggers.clone()))),
        }
    }

    /// Get the trigger info for a peripheral based on the MCU and peripheral name.
    ///
    /// Parameters:
    /// - mcu_name: the full name of the MCU (e.g., "STM32WB55RG")
    /// - peripheral: the name of the peripheral (e.g., "USART1")
    pub fn peripheral_trigger_info(&self, mcu_name: &str, peripheral: &str) -> Option<&[Trigger]> {
        if let Some(trigger) = self.map.get(&format!("{mcu_name}:{peripheral}")).next().map(|v| &**v) {
            assert!(
                !peripheral.contains("COMMON"),
                "rule applied trigger to a common peripheral"
            );

            Some(trigger)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wba_adc4_regular_triggers() {
        let triggers = Triggers::new();

        let adc4_triggers = triggers
            .peripheral_trigger_info("STM32WBA65RI", "ADC4")
            .expect("STM32WBA65RI:ADC4 should have trigger info");

        // ADC4 on WBA5/WBA6 has no injected channels, so its regular-trigger signal
        // must be named ADC_EXT_TRG (not ADC_TRG) to match the embassy-stm32 build.rs
        // mapping that generates `RegularTrigger<ADC4>` impls.
        assert!(
            adc4_triggers
                .iter()
                .any(|t| t.signal == "ADC_EXT_TRG0" && t.source == "TIM1_TRGO2")
        );
    }
}
