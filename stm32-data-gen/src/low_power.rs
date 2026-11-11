use stm32_data_serde::chip::core::peripheral::rcc::StopMode;

use crate::config;
use crate::util::new_regex_map;

/// The stop-mode limit of each peripheral, keyed by `{mcu}:{peripheral}`.
///
/// The rules themselves are data, not code: they live in `data/low_power.yaml`
/// and are read through [`config::STOP_MODE_RULES`], so a wrong mapping can be
/// fixed without rebuilding the generator.
pub struct ChipStopModes {
    /// First matching rule wins, so the rules keep their file order.
    map: regex_map::RegexMap<StopMode>,
}

impl ChipStopModes {
    pub fn new() -> Self {
        Self {
            map: new_regex_map(
                config::STOP_MODE_RULES
                    .iter()
                    .map(|rule| (rule.pattern.as_str(), rule.stop_mode.clone())),
            ),
        }
    }

    /// Get the stop mode limit for a peripheral based on the MCU and peripheral name.
    /// Determines the lowest possible stop mode when a peripheral is enabled.
    ///
    /// Parameters:
    /// - mcu_name: the full name of the MCU (e.g., "STM32WB55RG")
    /// - peripheral: the name of the peripheral (e.g., "USART1")
    pub(crate) fn peripheral_stop_mode_info(&self, mcu_name: &str, peripheral: &str) -> Option<StopMode> {
        self.map.get(&format!("{mcu_name}:{peripheral}")).next().cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_peripheral_stop_mode_info() {
        // The config files are resolved relative to the repository root, but
        // `cargo test` runs with the package root as the working directory.
        crate::util::chdir_repo_root();

        let chip_stop_modes = ChipStopModes::new();

        // MCU independent rule for RTC
        assert_eq!(
            chip_stop_modes.peripheral_stop_mode_info("my-test-mcu", "RTC"),
            Some(StopMode::Standby)
        );

        // No rule for this but starting with LP prefix, so assumed to be Stop2
        assert_eq!(
            chip_stop_modes.peripheral_stop_mode_info("my-test-mcu", "LPTIM2"),
            Some(StopMode::Stop2)
        );

        // MCU independent rule for RTC. Must match RTC exactly
        assert_eq!(
            chip_stop_modes.peripheral_stop_mode_info("my-test-mcu", "RTC1"),
            None
        );

        // Rule covering all STM32WB55 for LPTIM1
        assert_eq!(
            chip_stop_modes.peripheral_stop_mode_info("STM32WB55RG", "LPTIM1"),
            Some(StopMode::Standby)
        );

        // U0 LPTIMs run in Stop 2; the specific rule must beat the generic LP rule
        assert_eq!(
            chip_stop_modes.peripheral_stop_mode_info("STM32U083RC", "LPTIM1"),
            Some(StopMode::Standby)
        );
    }
}
