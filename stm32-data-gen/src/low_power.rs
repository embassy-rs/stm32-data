use stm32_data_serde::chip::core::peripheral::rcc::StopMode;

use crate::util::RegexMap;

/// Get the stop mode limit for a peripheral based on the MCU and peripheral name.
/// Determines the lowest possible stop mode when a peripheral is enabled.
///
/// Parameters:
/// - mcu_name: the full name of the MCU (e.g., "STM32WB55RG")
/// - peripheral: the name of the peripheral (e.g., "USART1")
pub(crate) fn peripheral_stop_mode_info(mcu_name: &str, peripheral: &str) -> Option<StopMode> {
    /// Regexmap where the key is mcu_name:peripheral and the value is the stop mode.
    /// Example: STM32WB55RG:USART1 -> StopMode::Stop2
    #[rustfmt::skip]
    static STOP_MODE_OVERRIDE_RULES: RegexMap<StopMode> = RegexMap::new(&[
        (r"^STM32WB55.*:LPTIM1", StopMode::Standby),
        (r"^STM32WB55.*:USART1", StopMode::Stop2),
        (r"^STM32WB55.*:LPUART1", StopMode::Standby),
        (r"^STM32WB55.*:I2C1", StopMode::Stop2),
        (r"^STM32WB55.*:I2C3", StopMode::Standby),
        (r"^STM32WLE5.*:LPUART1", StopMode::Standby),
        (r"^STM32WLE5.*:I2C1", StopMode::Stop2),
        (r"^STM32WLE5.*:I2C2", StopMode::Stop2),
        (r"^STM32WLE5.*:I2C3", StopMode::Standby),
        (r"^STM32WLE5.*:LPTIM1", StopMode::Standby),
        (r"^STM32WLE5.*:SUBGHZSPI", StopMode::Stop2),

        // __ATTENTION__: Keep these rules at the bottom to grant precedence to the more specific rules above
        // Every peripheral with LP prefix is assumed to be able enter up to Stop1 mode
        (r".*:LP.*", StopMode::Stop2), 
        // The RTC peripheral is assumed to be able to enter up to Stop2 mode
        (r".*:RTC", StopMode::Standby),
    ]);

    STOP_MODE_OVERRIDE_RULES
        .get(&format!("{mcu_name}:{peripheral}"))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_peripheral_stop_mode_info() {
        // MCU independent rule for RTC
        assert_eq!(peripheral_stop_mode_info("my-test-mcu", "RTC"), Some(StopMode::Standby));

        // No rule for this but starting with LP prefix, so assumed to be Stop2
        assert_eq!(
            peripheral_stop_mode_info("my-test-mcu", "LPTIM2"),
            Some(StopMode::Stop2)
        );

        // MCU independent rule for RTC. Must match RTC exactly
        assert_eq!(peripheral_stop_mode_info("my-test-mcu", "RTC1"), None);

        // Rule covering all STM32WB55 for LPTIM1
        assert_eq!(
            peripheral_stop_mode_info("STM32WB55RG", "LPTIM1"),
            Some(StopMode::Standby)
        );
    }
}
