use std::collections::HashMap;

use serde::Deserialize;
use stm32_data_serde::chip::Power;
use stm32_data_serde::chip::core::peripheral::rcc::LowPower;
use stm32_data_serde::chip::power::Mode;

use crate::config;
use crate::util::new_regex_map;

/// Low-power chip descriptors and per-peripheral Stop capabilities.
///
/// The rules live in `data/low_power.yaml` (`LOW_POWER`), supplemented by
/// DFP `*_EXTI.json` wake-line descriptors under `sources/cubeprogdb2/`.
pub struct ChipLowPower {
    /// Chip name -> optional Power descriptor (first match wins).
    chips: regex_map::RegexMap<Option<config::ChipPowerSpec>>,
    /// `{mcu}:{peripheral}` -> capability rule (first match wins).
    peripherals: regex_map::RegexMap<LowPower>,
    /// `(die, peripheral)` -> EXTI line number from DFP `*_EXTI.json`.
    dfp_wake_exti: HashMap<(String, String), u8>,
}

#[derive(Deserialize)]
struct DfpExtiFile {
    lines: Vec<DfpExtiLine>,
}

#[derive(Deserialize)]
struct DfpExtiLine {
    line_id: String,
    #[serde(default)]
    interconnect: Vec<DfpExtiInterconnect>,
}

#[derive(Deserialize)]
struct DfpExtiInterconnect {
    instance: Option<String>,
    source: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
}

fn load_dfp_wake_exti() -> HashMap<(String, String), u8> {
    let mut map = HashMap::new();
    let Ok(paths) = glob::glob("sources/cubeprogdb2/*/Descriptors/peripherals/*_EXTI.json") else {
        return map;
    };
    for path in paths.flatten() {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(die_code) = stem.strip_suffix("_EXTI").and_then(|s| s.strip_prefix('D')) else {
            continue;
        };
        let die = format!("DIE{die_code}");
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<DfpExtiFile>(&text) else {
            continue;
        };
        for line in parsed.lines {
            let Some(line_num) = line.line_id.strip_prefix("EXTI").and_then(|n| n.parse::<u8>().ok()) else {
                continue;
            };
            for ic in line.interconnect {
                if ic.kind.as_deref() != Some("peripheral") {
                    continue;
                }
                let Some(instance) = ic.instance else {
                    continue;
                };
                let is_wkup = ic
                    .source
                    .as_deref()
                    .is_some_and(|s| s.ends_with("_WKUP") || s.ends_with("_WKUP_IT"));
                if is_wkup {
                    map.entry((die.clone(), instance)).or_insert(line_num);
                }
            }
        }
    }
    map
}

impl ChipLowPower {
    pub fn new() -> Self {
        Self {
            chips: new_regex_map(
                config::LOW_POWER
                    .chips
                    .iter()
                    .map(|rule| (rule.pattern.as_str(), rule.power.clone())),
            ),
            peripherals: new_regex_map(
                config::LOW_POWER
                    .peripherals
                    .iter()
                    .map(|rule| (rule.pattern.as_str(), rule.to_low_power())),
            ),
            dfp_wake_exti: load_dfp_wake_exti(),
        }
    }

    pub(crate) fn chip_power(&self, mcu_name: &str, stopwuck: bool) -> Option<Power> {
        let spec = self
            .chips
            .get(mcu_name)
            .next()
            .unwrap_or_else(|| panic!("no chip low-power rule matched {mcu_name}"))
            .as_ref()?;

        Some(Power {
            dialect: spec.dialect,
            stopwuck,
            modes: spec
                .modes
                .iter()
                .map(|m| Mode {
                    kind: m.kind,
                    selector: m.selector,
                    min_svos: m.min_svos,
                    surviving_clocks: m.surviving_clocks.clone(),
                    min_residency_us: m.min_residency_us,
                    exit_latency_us: m.exit_latency_us,
                })
                .collect(),
        })
    }

    pub(crate) fn peripheral_low_power(&self, mcu_name: &str, die: &str, peripheral: &str) -> LowPower {
        let mut lp = self
            .peripherals
            .get(&format!("{mcu_name}:{peripheral}"))
            .next()
            .cloned()
            .unwrap_or_default();

        if lp.wake_exti.is_none()
            && let Some(&line) = self.dfp_wake_exti.get(&(die.to_string(), peripheral.to_string()))
        {
            lp.wake_exti = Some(line);
        }

        lp
    }
}

#[cfg(test)]
mod tests {
    use stm32_data_serde::chip::power::{Dialect, ModeKind};

    use super::*;

    #[test]
    fn test_get_peripheral_low_power_info() {
        crate::util::chdir_repo_root();

        let lp = ChipLowPower::new();

        // MCU-independent rule for RTC
        let rtc = lp.peripheral_low_power("my-test-mcu", "DIE000", "RTC");
        assert!(rtc.modes.stop0 && rtc.modes.stop1 && rtc.modes.stop2 && rtc.modes.stop3);
        assert!(rtc.autonomous && rtc.wake_capable);

        // Generic LP* fallback allows Stop0 + Stop1
        let lptim2 = lp.peripheral_low_power("my-test-mcu", "DIE000", "LPTIM2");
        assert!(lptim2.modes.stop0 && lptim2.modes.stop1 && !lptim2.modes.stop2);

        // RTC rule must match RTC exactly
        let rtc1 = lp.peripheral_low_power("my-test-mcu", "DIE000", "RTC1");
        assert!(!rtc1.modes.any());

        // U0 LPTIMs run in Stop 2; specific rule beats generic LP rule
        let u0_lptim1 = lp.peripheral_low_power("STM32U083RC", "DIE459", "LPTIM1");
        assert!(u0_lptim1.modes.stop2 && u0_lptim1.wake_capable);

        // U5 autonomous SmartRun peripheral vs CPU-domain peripheral
        let u5_lpuart = lp.peripheral_low_power("STM32U585AI", "DIE482", "LPUART1");
        assert!(u5_lpuart.modes.stop2 && u5_lpuart.autonomous && u5_lpuart.wake_capable);
        let u5_gpdma = lp.peripheral_low_power("STM32U585AI", "DIE482", "GPDMA1");
        assert!(u5_gpdma.modes.stop1 && !u5_gpdma.modes.stop2 && !u5_gpdma.autonomous);

        // USB OTG Stop capabilities and EXTI lines across families
        let wba_otg = lp.peripheral_low_power("STM32WBA65RI", "DIE494", "USB_OTG_HS");
        assert!(wba_otg.modes.stop0 && !wba_otg.modes.stop1 && !wba_otg.modes.stop2 && wba_otg.wake_capable);
        let u5_otg_hs = lp.peripheral_low_power("STM32U5A5ZJ", "DIE481", "USB_OTG_HS");
        assert!(u5_otg_hs.modes.stop0 && !u5_otg_hs.modes.stop1 && u5_otg_hs.wake_capable);
        let u5_otg_fs = lp.peripheral_low_power("STM32U585AI", "DIE482", "USB_OTG_FS");
        assert!(u5_otg_fs.modes.stop0 && u5_otg_fs.modes.stop1 && !u5_otg_fs.modes.stop2 && u5_otg_fs.wake_capable);
        let l4_otg_fs = lp.peripheral_low_power("STM32L476RG", "DIE415", "USB_OTG_FS");
        assert!(l4_otg_fs.modes.stop1 && !l4_otg_fs.modes.stop2 && l4_otg_fs.wake_exti == Some(17));
        let f4_otg_fs = lp.peripheral_low_power("STM32F407VG", "DIE413", "USB_OTG_FS");
        assert!(f4_otg_fs.modes.stop1 && f4_otg_fs.wake_exti == Some(18));
        let f4_otg_hs = lp.peripheral_low_power("STM32F407VG", "DIE413", "USB_OTG_HS");
        assert!(f4_otg_hs.modes.stop1 && f4_otg_hs.wake_exti == Some(20));
        let h5_otg_hs = lp.peripheral_low_power("STM32H5E5AI", "DIE478", "USB_OTG_HS");
        assert!(h5_otg_hs.modes.stop0 && !h5_otg_hs.modes.stop1 && h5_otg_hs.wake_capable);

        // Chip power descriptors
        let g4 = lp.chip_power("STM32G474RE", false).unwrap();
        assert_eq!(g4.dialect, Dialect::LpmsScr);
        assert_eq!(
            g4.modes.iter().map(|m| m.kind).collect::<Vec<_>>(),
            vec![ModeKind::Stop0, ModeKind::Stop1]
        );

        let mp1 = lp.chip_power("STM32MP157C", false);
        assert!(mp1.is_none());
    }
}
