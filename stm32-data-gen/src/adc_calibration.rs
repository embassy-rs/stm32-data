use serde::Deserialize;
use stm32_data_serde::chip::AdcCalibration;

use crate::config::load_yaml;
use crate::util::new_regex_map;

#[derive(Deserialize)]
struct Rule {
    #[serde(rename = "match")]
    pattern: String,
    calibration: Option<AdcCalibration>,
}

/// Chip-level ADC factory calibration rules loaded from `data/adc_calibration.yaml`.
pub struct ChipAdcCalibration {
    map: regex_map::RegexMap<Option<AdcCalibration>>,
}

impl ChipAdcCalibration {
    pub fn new() -> Self {
        let rules: Vec<Rule> = load_yaml("data/adc_calibration.yaml");
        Self {
            map: new_regex_map(rules.into_iter().map(|r| (r.pattern, r.calibration))),
        }
    }

    pub fn get(&self, chip_name: &str) -> Option<AdcCalibration> {
        *self
            .map
            .get(chip_name)
            .next()
            .unwrap_or_else(|| panic!("no adc_calibration rule matched chip {chip_name}"))
    }
}

#[cfg(test)]
mod tests {
    use stm32_data_serde::chip::TempCalibration;

    use super::*;

    #[test]
    fn test_adc_calibration_rules() {
        crate::util::chdir_repo_root();
        let cal = ChipAdcCalibration::new();

        assert_eq!(cal.get("STM32MP157CAA"), None);
        assert_eq!(cal.get("STM32F103C8").unwrap().vrefint, None);
        assert!(matches!(
            cal.get("STM32C031C6").unwrap().temp,
            Some(TempCalibration::OnePoint(_))
        ));
        assert!(matches!(
            cal.get("STM32F030C6").unwrap().temp,
            Some(TempCalibration::OnePoint(_))
        ));
        assert!(matches!(
            cal.get("STM32F072RB").unwrap().temp,
            Some(TempCalibration::TwoPoint(_))
        ));
        assert_eq!(cal.get("STM32U585AI").unwrap().vrefint.unwrap().cal_bits, 14);
        assert_eq!(cal.get("STM32H743ZI").unwrap().vrefint.unwrap().cal_bits, 16);
        assert_eq!(cal.get("STM32H7S3L8").unwrap().vrefint.unwrap().cal_bits, 12);
        assert_eq!(cal.get("STM32L010C6").unwrap().temp, None);
    }
}
