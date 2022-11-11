use std::collections::HashMap;

use crate::regex;

#[derive(Debug)]
pub struct PeripheralToClock(
    HashMap<(String, String, String), HashMap<String, stm32_data_serde::chip::core::peripheral::Rcc>>,
);

impl PeripheralToClock {
    pub fn parse() -> anyhow::Result<Self> {
        let mut peripheral_to_clock = HashMap::new();
        for f in glob::glob("data/registers/rcc_*")? {
            let f = f?;
            let ff = f
                .file_name()
                .unwrap()
                .to_string_lossy()
                .strip_prefix("rcc_")
                .unwrap()
                .strip_suffix(".yaml")
                .unwrap()
                .to_string();
            let mut family_clocks = HashMap::new();
            let y: chiptool::ir::IR = serde_yaml::from_str(&std::fs::read_to_string(f)?)?;
            for (reg, body) in &y.fieldsets {
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
                            // Timers are a bit special, they may have a x2 freq
                            let peri_clock = {
                                if regex!(r"^TIM\d+$").is_match(peri) {
                                    format!("{clock}_TIM")
                                } else {
                                    clock.to_string()
                                }
                            };

                            let mut reset = None;
                            if let Some(rstr) = y.fieldsets.get(&reg.replace("ENR", "RSTR")) {
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
            peripheral_to_clock.insert(("rcc".to_string(), ff, "RCC".to_string()), family_clocks);
        }

        Ok(Self(peripheral_to_clock))
    }

    pub fn match_peri_clock(
        &self,
        rcc_block: (String, String, String),
        peri_name: &str,
    ) -> Option<&stm32_data_serde::chip::core::peripheral::Rcc> {
        let clocks = self.0.get(&rcc_block)?;
        if let Some(res) = clocks.get(peri_name) {
            Some(res)
        } else if let Some(peri_name) = peri_name.strip_suffix('1') {
            self.match_peri_clock(rcc_block, peri_name)
        } else {
            None
        }
    }
}
