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

        // Group signals per mux table: strip a trailing input index ("ADC_EXT_TRG12" ->
        // "ADC_EXT_TRG", "TIMX_TI1_IN2" -> "TIMX_TI1", "TIMX_ITR_IN2" -> "TIMX_ITR").
        // A source may legitimately feed several different muxes of one peripheral
        // (e.g. COMP2_OUT into both TI1 and TI2), so only duplicates within one
        // table are rejected.
        let trigger_expr = Regex::new(r"^(.+?)(?:_IN)?(\d+)$").unwrap();

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

                // Every source must be classified for per-die trimming, so a
                // new fragment with a new source spelling fails loudly here
                // instead of silently never being trimmed.
                if matches!(source_origin(&trigger.source), SourceOrigin::Unknown) {
                    panic!(
                        "trigger: unclassified source {} in rule {} — add it to source_origin() in stm32-data-gen/src/trigger.rs",
                        trigger.source, rule.match_
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

/// Where a trigger `source` originates from, for per-die trimming
/// (see `data/triggers/CONVENTIONS.md`: the rules keep the full RM column
/// for the whole line; the generator drops sources whose originating
/// peripheral is absent from the chip being generated).
#[derive(Debug, PartialEq, Eq)]
pub enum SourceOrigin<'a> {
    /// Not tied to a chip peripheral instance (system-level event such as
    /// `PVD` or `CORTEX_M4_LOCKUP`): never trimmed.
    System,
    /// Requires a peripheral whose name equals this (e.g. `TIM1_TRGO` -> `TIM1`).
    Exact(&'a str),
    /// Requires a peripheral whose name starts with this prefix (e.g.
    /// `HRTIM_ADC_TRG1` -> `HRTIM1`, `GPDMA_CH0_TC` -> `GPDMA1`).
    Prefix(&'a str),
    /// Requires any one of these peripherals (`COMP1_OUT_OR_COMP2_OUT`).
    AnyOf(&'a [&'a str]),
    /// Not recognized: kept conservatively, but `Triggers::new` rejects
    /// sources classified this way so every source in `rules.yaml` is
    /// classified explicitly.
    Unknown,
}

/// Classify a trigger `source` for per-die trimming.
///
/// `head` is the token before the first `_` (or the whole source); the
/// numbered families (`TIM1_TRGO`, `LPTIM2_IN1`, `COMP3_OUT`,
/// `DFSDM1_BREAK0`, `ADC4_AWD1`) map to an exact peripheral name, families
/// whose sources omit the instance number (`HRTIM*`, `TAMP*`, `GPDMA*`,
/// `LPTIMOUT`) map to a name prefix, and system-level events are never
/// tied to a peripheral.
pub fn source_origin(source: &str) -> SourceOrigin<'_> {
    // System-level events: not produced by any peripheral instance.
    const SYSTEM: &[&str] = &[
        "PVD",
        "LSI",
        "LSE",
        "MCO",
        "CSS",
        "HSE32_HSECSS",
        "HSE_DIV32",
        "HSI16_DIV256",
        "SRAM_PARITY_ERROR",
        "FLASH_ECC_ERROR",
        "CORTEX_M4_LOCKUP",
        "CORTEX_M33_LOCKUP",
    ];
    if SYSTEM.contains(&source) {
        return SourceOrigin::System;
    }

    // Sources naming several alternative originating peripherals.
    match source {
        "COMP1_OUT_OR_COMP2_OUT" => return SourceOrigin::AnyOf(&["COMP1", "COMP2"]),
        "USB_SOF_SYNC" => return SourceOrigin::AnyOf(&["USB", "USB_OTG_FS", "USB_OTG_HS"]),
        "LPTIMOUT" => return SourceOrigin::Prefix("LPTIM"),
        _ => {}
    }

    let head = source.split('_').next().unwrap_or(source);

    // Families whose sources name the instance: `TIM1_TRGO` -> `TIM1`.
    for family in ["TIM", "LPTIM", "COMP", "DFSDM", "ADC"] {
        if let Some(rest) = head.strip_prefix(family) {
            if !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()) {
                return SourceOrigin::Exact(head);
            }
        }
    }

    // Families whose sources omit the instance number.
    if head.starts_with("HRTIM") {
        return SourceOrigin::Prefix("HRTIM");
    }
    if head.starts_with("TAMP") {
        return SourceOrigin::Prefix("TAMP");
    }
    if head.starts_with("GPDMA") {
        return SourceOrigin::Prefix("GPDMA");
    }
    if head.starts_with("LPDMA") {
        return SourceOrigin::Prefix("LPDMA");
    }
    // Single-instance peripherals named as they are in the chip data.
    if ["LTDC", "DMA2D", "GPU2D", "GFXTIM", "JPEG"].contains(&head) {
        return SourceOrigin::Exact(head);
    }
    if head == "DSI" {
        return SourceOrigin::Exact("DSIHOST");
    }
    // EXTI lines are produced by the single EXTI block, not `EXTI11` etc.
    if head.starts_with("EXTI") {
        return SourceOrigin::Exact("EXTI");
    }
    if head == "RTC" {
        return SourceOrigin::Exact("RTC");
    }

    SourceOrigin::Unknown
}

/// Per-die trimming: whether a trigger `source` is usable on a die whose
/// peripheral set is `peripherals` (the names in the generated chip JSON).
///
/// System-level and unrecognized sources are kept; see `source_origin`.
pub fn source_available(source: &str, peripherals: &HashSet<String>) -> bool {
    match source_origin(source) {
        SourceOrigin::System | SourceOrigin::Unknown => true,
        SourceOrigin::Exact(name) => peripherals.contains(name),
        SourceOrigin::Prefix(prefix) => peripherals.iter().any(|p| p.starts_with(prefix)),
        SourceOrigin::AnyOf(names) => names.iter().any(|n| peripherals.contains(*n)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c5_dac_tim6_trigger_selection() {
        let triggers = Triggers::new();
        for chip in ["STM32C531RC", "STM32C5A3ZG"] {
            let dac_triggers = triggers.peripheral_trigger_info(chip, "DAC1").unwrap();
            let tim6 = dac_triggers.iter().find(|t| t.source == "TIM6_TRGO").unwrap();
            assert_eq!(tim6.signal, "DAC_CHX_TRG6");
        }
    }

    #[test]
    fn u5_gpdma1_triggers() {
        let triggers = Triggers::new();
        let signal = |chip, source| {
            triggers
                .peripheral_trigger_info(chip, "GPDMA1")
                .unwrap()
                .iter()
                .find(|t| t.source == source)
                .map(|t| t.signal.as_str())
        };

        for chip in ["STM32U585CI", "STM32U5A5ZJ", "STM32U535CB"] {
            assert_eq!(signal(chip, "EXTI6_TRG"), Some("GPDMA_TRG6"));
        }
        // TIM3/4/5_TRGO only on the U59x/5Ax and U5Fx/5Gx (RM0456 Table 139).
        assert_eq!(signal("STM32U585CI", "TIM3_TRGO"), None);
        assert_eq!(signal("STM32U535CB", "TIM3_TRGO"), None);
        assert_eq!(signal("STM32U5A5ZJ", "TIM3_TRGO"), Some("GPDMA_TRG44"));
        assert_eq!(signal("STM32U585CI", "DMA2D_TC"), Some("GPDMA_TRG50"));
    }

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

    #[test]
    fn test_source_origin_classification() {
        assert_eq!(source_origin("TIM1_TRGO"), SourceOrigin::Exact("TIM1"));
        assert_eq!(source_origin("TIM16_OC1"), SourceOrigin::Exact("TIM16"));
        assert_eq!(source_origin("TIM24_TRGO"), SourceOrigin::Exact("TIM24"));
        assert_eq!(source_origin("LPTIM2_IN1"), SourceOrigin::Exact("LPTIM2"));
        assert_eq!(source_origin("COMP7_OUT"), SourceOrigin::Exact("COMP7"));
        assert_eq!(source_origin("DFSDM2_BREAK2"), SourceOrigin::Exact("DFSDM2"));
        assert_eq!(source_origin("ADC4_AWD1"), SourceOrigin::Exact("ADC4"));
        assert_eq!(source_origin("EXTI11_TRG"), SourceOrigin::Exact("EXTI"));
        assert_eq!(source_origin("RTC_WAKEUP"), SourceOrigin::Exact("RTC"));
        assert_eq!(source_origin("LPTIMOUT"), SourceOrigin::Prefix("LPTIM"));
        assert_eq!(source_origin("HRTIM_ADC_TRG1"), SourceOrigin::Prefix("HRTIM"));
        assert_eq!(source_origin("HRTIM1_ADCTRG1"), SourceOrigin::Prefix("HRTIM"));
        assert_eq!(source_origin("TAMP_TRG1"), SourceOrigin::Prefix("TAMP"));
        assert_eq!(source_origin("GPDMA_CH0_TC"), SourceOrigin::Prefix("GPDMA"));
        assert_eq!(
            source_origin("COMP1_OUT_OR_COMP2_OUT"),
            SourceOrigin::AnyOf(&["COMP1", "COMP2"])
        );
        assert_eq!(
            source_origin("USB_SOF_SYNC"),
            SourceOrigin::AnyOf(&["USB", "USB_OTG_FS", "USB_OTG_HS"])
        );
        assert_eq!(source_origin("PVD"), SourceOrigin::System);
        assert_eq!(source_origin("CORTEX_M4_LOCKUP"), SourceOrigin::System);
        assert_eq!(source_origin("TIMX_BOGUS"), SourceOrigin::Unknown);
    }

    #[test]
    fn test_source_available_trims_missing_peripherals() {
        let peris: HashSet<String> = ["TIM1", "ADC1", "COMP1", "HRTIM1", "LPTIM1", "EXTI", "RTC"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        assert!(source_available("TIM1_TRGO", &peris));
        assert!(!source_available("TIM8_TRGO", &peris));
        assert!(source_available("HRTIM_ADC_TRG1", &peris)); // prefix matches HRTIM1
        assert!(source_available("COMP1_OUT", &peris));
        assert!(!source_available("COMP2_OUT", &peris));
        assert!(source_available("COMP1_OUT_OR_COMP2_OUT", &peris)); // either candidate
        assert!(source_available("EXTI11_TRG", &peris));
        assert!(source_available("PVD", &peris)); // system: always available

        let empty = HashSet::new();
        assert!(!source_available("TIM1_TRGO", &empty));
        assert!(source_available("CORTEX_M33_LOCKUP", &empty));
    }
}
