use std::collections::HashMap;

use crate::normalize_peris::normalize_peri_name;
use crate::regex;

mod xml {
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct Ip {
        #[serde(rename = "Name")]
        pub name: String,
        #[serde(rename = "Version")]
        pub version: String,
        #[serde(rename = "GPIO_Pin")]
        pub gpio_pins: Vec<GpioPin>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct GpioPin {
        #[serde(rename = "Name")]
        pub name: String,
        #[serde(rename = "PinSignal", default)]
        pub pin_signals: Vec<PinSignal>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct PinSignal {
        #[serde(rename = "Name")]
        pub name: String,
        #[serde(rename = "SpecificParameter", default)]
        pub specific_parameter: Option<SpecificParameter>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct SpecificParameter {
        #[serde(rename = "PossibleValue")]
        pub possible_value: String,
    }
}

pub fn clean_pin(pin_name: &str) -> Option<String> {
    // Some H7 chips have additonal "_C" pins.
    Some(regex!(r"^P[A-Z]\d+(?:_C)?").find(pin_name)?.as_str().into())
}

#[derive(Debug, PartialEq, Eq)]
pub struct Af(pub HashMap<String, HashMap<String, Vec<stm32_data_serde::chip::core::peripheral::Pin>>>);

impl Af {
    pub fn parse() -> anyhow::Result<Self> {
        let mut af = HashMap::new();
        for f in glob::glob("sources/cubedb/mcu/IP/GPIO-*_gpio_v1_0_Modes.xml")? {
            let parsed: xml::Ip = quick_xml::de::from_str(&std::fs::read_to_string(f?)?)?;

            let ff = parsed.version.strip_suffix("_gpio_v1_0").unwrap().to_string();

            let mut peris = HashMap::<_, Vec<_>>::new();

            for pin in parsed.gpio_pins {
                // Cleanup pin name
                let Some(pin_name) = clean_pin(&pin.name) else { continue };

                // Extract AFs
                for signal in pin.pin_signals {
                    let Some((peri_name, signal_name)) = parse_signal_name(&signal.name) else {
                        continue;
                    };
                    let afn = if parsed.version.starts_with("STM32F1") {
                        None
                    } else {
                        let afn = signal.specific_parameter.unwrap();
                        let afn = afn
                            .possible_value
                            .split('_')
                            .nth(1)
                            .unwrap()
                            .strip_prefix("AF")
                            .unwrap()
                            .parse()
                            .unwrap();
                        Some(afn)
                    };
                    peris.entry(peri_name.to_string()).or_default().push(
                        stm32_data_serde::chip::core::peripheral::Pin {
                            pin: pin_name.clone(),
                            signal: signal_name.to_string(),
                            af: afn,
                        },
                    );
                }
            }

            for p in peris.values_mut() {
                p.sort_by_key(|p| pin_sort_key(&p.pin));
                p.dedup();
            }

            af.insert(ff, peris);
        }

        Ok(Self(af))
    }
}

pub fn parse_signal_name(signal_name: &str) -> Option<(&str, &str)> {
    let (peri_name, signal_name) = {
        if let Some(signal_name) = signal_name.strip_prefix("USB_OTG_FS_") {
            ("USB_OTG_FS", signal_name)
        } else if let Some(signal_name) = signal_name.strip_prefix("USB_OTG_HS_") {
            ("USB_OTG_HS", signal_name)
        } else {
            signal_name.split_once('_')?
        }
    };

    if signal_name.starts_with("EXTI") {
        return None;
    }
    if peri_name.starts_with("DEBUG") && signal_name.starts_with("SUBGHZSPI") {
        let (peri_name, signal_name) = signal_name.split_once('-').unwrap();

        Some((peri_name, signal_name.strip_suffix("OUT").unwrap_or(signal_name)))
    } else {
        Some((
            normalize_peri_name(peri_name),
            if signal_name.starts_with("RMII_") {
                signal_name.strip_prefix("RMII_").unwrap_or(signal_name)
            } else {
                signal_name.strip_prefix("MII_").unwrap_or(signal_name)
            },
        ))
    }
}

pub fn pin_sort_key(pin: &str) -> (char, u8) {
    let captures = regex!(r"^P([A-Z])(\d+)(?:_C)?")
        .captures(pin)
        .expect("Could not match regex on pin");
    let port = captures
        .get(1)
        .expect("Could not extract port")
        .as_str()
        .chars()
        .next()
        .expect("Empty port");
    let pin_number = captures
        .get(2)
        .expect("Could not extract pin number")
        .as_str()
        .parse::<u8>()
        .expect("Could not parse pin number to u8");
    (port, pin_number)
}
