use std::collections::HashMap;

use crate::regex;
use crate::registers::Registers;

#[derive(Debug)]
pub struct PeripheralToClock(
    HashMap<(String, String, String), HashMap<String, stm32_data_serde::chip::core::peripheral::Rcc>>,
);

impl PeripheralToClock {
    pub fn parse(registers: &Registers) -> anyhow::Result<Self> {
        let mut peripheral_to_clock = HashMap::new();

        for (rcc_name, ir) in &registers.registers {
            if let Some(rcc_name) = rcc_name.strip_prefix("rcc_") {
                let mut family_clocks = HashMap::new();
                for (reg, body) in &ir.fieldsets {
                    let key = format!("fieldset/{reg}");
                    if let Some(m) = regex!(r"^fieldset/((A[PH]B\d?)|GPIO)[LH]?ENR\d?$").captures(&key) {
                        let clock = m.get(1).unwrap().as_str();
                        let clock = match clock {
                            "AHB" => "AHB1",
                            "APB" => "APB1",
                            clock => clock,
                        };
                        for field in &body.fields {
                            if let Some(peri) = field.name.strip_suffix("EN") {
                                let peri = if peri == "RTCAPB" { "RTC" } else { peri };

                                // Timers are a bit special, they may have a x2 freq
                                let peri_clock = {
                                    if regex!(r"^TIM\d+$").is_match(peri) {
                                        format!("{clock}_TIM")
                                    } else {
                                        clock.to_string()
                                    }
                                };

                                let mut reset = None;
                                if let Some(rstr) = ir.fieldsets.get(&reg.replace("ENR", "RSTR")) {
                                    if let Some(_field) =
                                        rstr.fields.iter().find(|field| field.name == format!("{peri}RST"))
                                    {
                                        reset = Some(stm32_data_serde::chip::core::peripheral::rcc::Reset {
                                            register: reg.replace("ENR", "RSTR"),
                                            field: format!("{peri}RST"),
                                        });
                                    }
                                }

                                let res = stm32_data_serde::chip::core::peripheral::Rcc {
                                    clock: peri_clock,
                                    enable: stm32_data_serde::chip::core::peripheral::rcc::Enable {
                                        register: reg.clone(),
                                        field: field.name.clone(),
                                    },
                                    reset,
                                };

                                family_clocks.insert(peri.to_string(), res);
                            }
                        }
                    }
                }
                peripheral_to_clock.insert(
                    ("rcc".to_string(), rcc_name.to_string(), "RCC".to_string()),
                    family_clocks,
                );
            }
        }

        Ok(Self(peripheral_to_clock))
    }

    pub fn match_peri_clock(
        &self,
        rcc_block: &(String, String, String),
        peri_name: &str,
    ) -> Option<&stm32_data_serde::chip::core::peripheral::Rcc> {
        const PERI_OVERRIDE: &[(&str, &[&str])] = &[("DCMI", &["DCMI_PSSI"]), ("PSSI", &["DCMI_PSSI"])];

        let clocks = self.0.get(rcc_block)?;
        if peri_name.starts_with("ADC") && !peri_name.contains("COMMON") {
            return self.match_adc_peri_clock(clocks, peri_name);
        }
        if let Some(res) = clocks.get(peri_name) {
            Some(res)
        } else if let Some(peri_name) = peri_name.strip_suffix('1') {
            self.match_peri_clock(rcc_block, peri_name)
        } else if let Some((_, rename)) = PERI_OVERRIDE.iter().find(|(n, _)| *n == peri_name) {
            for n in *rename {
                if let Some(res) = self.match_peri_clock(rcc_block, n) {
                    return Some(res);
                }
            }
            None
        } else {
            None
        }
    }

    fn match_adc_peri_clock<'a>(
        &'a self,
        clocks: &'a HashMap<String, stm32_data_serde::chip::core::peripheral::Rcc>,
        peri_name: &str,
    ) -> Option<&stm32_data_serde::chip::core::peripheral::Rcc> {
        // Direct match
        if clocks.contains_key(peri_name) {
            return clocks.get(peri_name);
        }

        // Paired match based on odd/even
        if let Some(digit_char) = peri_name.chars().last() {
            if let Some(digit) = digit_char.to_digit(10) {
                let paired = if digit % 2 == 1 {
                    format!("ADC{}{}", digit, digit + 1)
                } else {
                    format!("ADC{}{}", digit - 1, digit)
                };

                if clocks.contains_key(paired.as_str()) {
                    return clocks.get(paired.as_str());
                }
            }
        }

        // If adc is 3, 4, or 5, check for ADC345
        if (peri_name == "ADC3" || peri_name == "ADC4" || peri_name == "ADC5") && clocks.contains_key("ADC345") {
            return clocks.get("ADC345");
        }

        // Look for bare ADC clock register
        if clocks.contains_key("ADC") {
            return clocks.get("ADC");
        }

        None
    }
}
