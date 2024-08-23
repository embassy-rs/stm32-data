use std::collections::HashMap;

use anyhow::Context;

use crate::normalize_peris::normalize_peri_name;

mod xml {
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct Ip {
        #[serde(rename = "Name")]
        pub name: String,
        #[serde(rename = "Version")]
        pub version: String,
        #[serde(rename = "RefParameter")]
        pub ref_parameters: Vec<RefParameter>,
        #[serde(rename = "RefMode")]
        pub ref_modes: Vec<RefMode>,
        #[serde(rename = "ModeLogicOperator")]
        pub mode_logic_operator: ModeLogicOperator,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct RefMode {
        #[serde(rename = "Name")]
        pub name: String,
        #[serde(rename = "BaseMode")]
        pub base_mode: Option<String>,
        #[serde(rename = "Parameter")]
        pub parameters: Vec<Parameter>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct Parameter {
        #[serde(rename = "Name")]
        pub name: String,
        #[serde(rename = "PossibleValue", default)]
        pub possible_values: Vec<String>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct RefParameter {
        #[serde(rename = "Name")]
        pub name: String,
        #[serde(rename = "PossibleValue", default)]
        pub possible_values: Vec<PossibleValue>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct PossibleValue {
        #[serde(rename = "Comment")]
        pub comment: String,
        #[serde(rename = "Value")]
        pub value: String,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct ModeLogicOperator {
        #[serde(rename = "Name")]
        pub name: String,
        #[serde(rename = "Mode")]
        pub modes: Vec<Mode>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct Mode {
        #[serde(rename = "Name")]
        pub name: String,
        #[serde(rename = "ModeLogicOperator", default)]
        pub mode_logic_operator: Option<ModeLogicOperator>,
    }
}

#[derive(Debug, PartialEq)]
pub struct ChipDma {
    pub peripherals: HashMap<String, Vec<stm32_data_serde::chip::core::peripheral::DmaChannel>>,
    pub channels: Vec<stm32_data_serde::chip::core::DmaChannels>,
}

#[derive(Debug)]
pub struct DmaChannels(pub HashMap<String, ChipDma>);

impl DmaChannels {
    pub fn parse() -> anyhow::Result<Self> {
        let mut dma_channels = HashMap::new();
        for f in glob::glob("sources/cubedb/mcu/IP/DMA*Modes.xml")?
            .chain(glob::glob("sources/cubedb/mcu/IP/BDMA*Modes.xml")?)
        {
            let f = f?;
            if f.to_string_lossy().contains("DMAMUX") {
                continue;
            }
            let parsed: xml::Ip = quick_xml::de::from_str(&std::fs::read_to_string(&f)?).context(format!("{:?}", f))?;

            let ff = parsed.version.clone();
            let is_explicitly_bdma = match parsed.name.as_str() {
                "DMA" | "DMA2D" => false,
                "BDMA" | "BDMA1" | "BDMA2" => true,
                name => panic!("Unrecognized DMA name: {name}"),
            };

            let mut chip_dma = ChipDma {
                peripherals: HashMap::new(),
                channels: Vec::new(),
            };

            for dma in parsed.mode_logic_operator.modes {
                let dma_peri_name = dma.name.clone();
                if dma_peri_name.contains(" Context") {
                    continue;
                }
                let channels = dma.mode_logic_operator.unwrap().modes;
                if channels.len() == 1 {
                    // ========== CHIP WITH DMAMUX

                    let dmamux_file = {
                        if ff.starts_with("STM32L4P") {
                            "L4PQ"
                        } else if ff.starts_with("STM32L4S") {
                            "L4RS"
                        } else {
                            &ff[5..7]
                        }
                    };

                    let dmamux = match is_explicitly_bdma {
                        true => "DMAMUX2",
                        false => "DMAMUX1",
                    };

                    let mut mfs: Vec<_> = glob::glob(&format!("data/dmamux/{dmamux_file}_*.yaml"))?
                        .map(Result::unwrap)
                        .collect();
                    mfs.sort();
                    for mf in mfs {
                        let y: HashMap<String, u8> = serde_yaml::from_str(&std::fs::read_to_string(&mf)?)?;

                        let mf = mf.file_name().unwrap().to_string_lossy();
                        let (_, req_dmamux) = mf.strip_suffix(".yaml").unwrap().split_once('_').unwrap(); // DMAMUX1 or DMAMUX2

                        if req_dmamux == dmamux {
                            for (request_name, request_num) in y {
                                let parts: Vec<_> = request_name.split('_').collect();
                                let target_peri_name = parts[0];

                                let target_peri_name = match target_peri_name {
                                    "SPDIF" => "SPDIFRX1",
                                    x => x,
                                };

                                let request = {
                                    if parts.len() < 2 {
                                        target_peri_name
                                    } else {
                                        parts[1]
                                    }
                                };
                                chip_dma
                                    .peripherals
                                    .entry(normalize_peri_name(target_peri_name).to_string())
                                    .or_default()
                                    .push(stm32_data_serde::chip::core::peripheral::DmaChannel {
                                        signal: request.to_string(),
                                        channel: None,
                                        dmamux: Some(req_dmamux.to_string()),
                                        request: Some(request_num),
                                        dma: None,
                                    })
                            }
                        }
                    }

                    let mut dmamux_channel = 0;
                    for n in dma_peri_name.split(',') {
                        let n = n.trim();
                        let re = regex::Regex::new(&format!(".*{n}{}", r"_(Channel|Stream)\[(\d+)-(\d+)\]")).unwrap();
                        if let Some(result) = re.captures(&channels[0].name) {
                            let low: u8 = result.get(2).unwrap().as_str().parse()?;
                            let high: u8 = result.get(3).unwrap().as_str().parse()?;
                            for i in low..=high {
                                chip_dma.channels.push(stm32_data_serde::chip::core::DmaChannels {
                                    name: format!("{n}_CH{i}"),
                                    dma: n.to_string(),
                                    // Make sure all channels numbers start at 0
                                    channel: i - low,
                                    dmamux: Some(dmamux.to_string()),
                                    dmamux_channel: Some(dmamux_channel),
                                    supports_2d: None,
                                });
                                dmamux_channel += 1;
                            }
                        }
                    }
                } else {
                    // ========== CHIP WITHOUT DMAMUX

                    // see if we can scrape out requests
                    let mut requests = HashMap::<String, u8>::new();
                    for block in parsed
                        .ref_modes
                        .iter()
                        .filter(|x| x.base_mode == Some("DMA_Request".to_string()))
                    {
                        let name = block.name.clone();
                        // Depending on the chip, the naming is "Channel" or "Request"...
                        if let Some(request_num) = block
                            .parameters
                            .iter()
                            .find(|x| x.name == "Channel" || x.name == "Request")
                        {
                            assert_eq!(request_num.possible_values.len(), 1);
                            let request_num = request_num.possible_values[0].clone();
                            if request_num.starts_with("BDMA1_REQUEST_") {
                                continue;
                            }
                            let request_num = request_num
                                .strip_prefix("DMA_CHANNEL_")
                                .or_else(|| request_num.strip_prefix("DMA_REQUEST_"))
                                .unwrap();
                            requests.insert(name, request_num.parse().unwrap());
                        }
                    }

                    let mut channel_names: Vec<u8> = Vec::new();

                    for channel in channels {
                        let channel_name = channel.name;
                        let (_, channel_name) = channel_name.split_once('_').unwrap();
                        let channel_name = channel_name
                            .strip_prefix("Channel")
                            .or_else(|| channel_name.strip_prefix("Stream"))
                            .unwrap();

                        channel_names.push(channel_name.parse().unwrap());
                        chip_dma.channels.push(stm32_data_serde::chip::core::DmaChannels {
                            name: format!("{dma_peri_name}_CH{channel_name}"),
                            dma: dma_peri_name.clone(),
                            channel: channel_name.parse().unwrap(),
                            dmamux: None,
                            dmamux_channel: None,
                            supports_2d: None,
                        });
                        for target in channel.mode_logic_operator.unwrap().modes {
                            let original_target_name = target.name;
                            let parts: Vec<_> = original_target_name.split(':').collect();
                            let target_name = parts[0];

                            //  Chips with single DAC refer to channels by DAC1/DAC2
                            let target_name = match target_name {
                                "DAC1" => "DAC_CH1",
                                "DAC2" => "DAC_CH2",
                                x => x,
                            };

                            let parts: Vec<_> = target_name.split('_').collect();
                            let target_peri_name = parts[0];
                            let target_requests = {
                                if parts.len() < 2 {
                                    vec![target_peri_name]
                                } else {
                                    target_name.split('_').nth(1).unwrap().split('/').collect()
                                }
                            };
                            if target_name != "MEMTOMEM" {
                                let target_peri_name = match target_peri_name {
                                    "LPUART" => "LPUART1",
                                    "SPDIF" => "SPDIFRX1",
                                    x => x,
                                };
                                for request in target_requests {
                                    assert!(!request.contains(':'));
                                    let entry = stm32_data_serde::chip::core::peripheral::DmaChannel {
                                        signal: request.to_string(),
                                        channel: Some(format!("{dma_peri_name}_CH{channel_name}")),
                                        dmamux: None,
                                        request: requests.get(&original_target_name).copied(),
                                        dma: None,
                                    };
                                    chip_dma
                                        .peripherals
                                        .entry(normalize_peri_name(target_peri_name).to_string())
                                        .or_default()
                                        .push(entry);
                                }
                            }
                        }
                    }

                    // Make sure all channels numbers start at 0
                    if channel_names.iter().min().unwrap() != &0 {
                        for ch in &mut chip_dma.channels {
                            if ch.dma == dma_peri_name {
                                ch.channel -= 1;
                            }
                        }
                    }
                }
            }

            dma_channels.insert(ff, chip_dma);
        }

        // GPDMA

        for (file, instance, version, count, count_2d) in [
            ("H5_GPDMA.yaml", "GPDMA1", "STM32H5_dma3_Cube", 8, 2),
            ("H5_GPDMA.yaml", "GPDMA2", "Instance2_STM32H5_dma3_Cube", 8, 2),
            ("U5_GPDMA1.yaml", "GPDMA1", "STM32U5_dma3_Cube", 16, 4),
            ("U5_LPDMA.yaml", "LPDMA1", "STM32U5_dma3_Cube", 4, 0),
            ("WBA_GPDMA1.yaml", "GPDMA1", "STM32WBA_dma3_Cube", 8, 0),
            ("H7RS_GPDMA.yaml", "GPDMA1", "STM32H7RS_dma3_Cube", 16, 4),
            ("H7RS_HPDMA.yaml", "HPDMA1", "STM32H7RS_dma3_Cube", 16, 4),
        ] {
            let mut chip_dma = ChipDma {
                peripherals: HashMap::new(),
                channels: Vec::new(),
            };

            let parsed: HashMap<String, u8> =
                serde_yaml::from_str(&std::fs::read_to_string(format!("data/dmamux/{file}"))?)?;

            for (request_name, request_num) in parsed {
                let parts: Vec<_> = request_name.split('_').collect();
                let target_peri_name = parts[0];
                let request = {
                    if parts.len() < 2 {
                        target_peri_name
                    } else {
                        parts[1]
                    }
                };
                chip_dma
                    .peripherals
                    .entry(normalize_peri_name(target_peri_name).to_string())
                    .or_default()
                    .push(stm32_data_serde::chip::core::peripheral::DmaChannel {
                        signal: request.to_string(),
                        dma: Some(instance.to_string()),
                        channel: None,
                        dmamux: None,
                        request: Some(request_num),
                    });
            }

            for i in 0..count {
                chip_dma.channels.push(stm32_data_serde::chip::core::DmaChannels {
                    name: format!("{instance}_CH{i}"),
                    dma: instance.to_string(),
                    channel: i,
                    dmamux: None,
                    dmamux_channel: None,
                    supports_2d: Some(i >= count - count_2d),
                });
            }

            dma_channels.insert(format!("{version}:{instance}"), chip_dma);
        }

        Ok(Self(dma_channels))
    }
}
