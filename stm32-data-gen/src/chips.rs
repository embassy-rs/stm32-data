use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use stm32_data_serde::chip::core::peripheral::Pin;

use super::*;

mod xml {
    use serde::Deserialize;

    #[derive(Clone, Debug, Deserialize, PartialEq)]
    pub struct Mcu {
        #[serde(rename = "Family")]
        pub family: String,
        #[serde(rename = "Line")]
        pub line: String,
        #[serde(rename = "Die")]
        pub die: String,
        #[serde(rename = "RefName")]
        pub ref_name: String,
        #[serde(rename = "Package")]
        pub package: String,
        #[serde(rename = "Core")]
        pub cores: Vec<String>,
        #[serde(rename = "Ram")]
        pub rams: Vec<u32>,
        #[serde(rename = "Flash")]
        pub flashs: Vec<u32>,
        #[serde(rename = "IP")]
        pub ips: Vec<Ip>,
        #[serde(rename = "Pin")]
        pub pins: Vec<Pin>,
    }

    #[derive(Clone, Debug, Deserialize, PartialEq)]
    pub struct Pin {
        #[serde(rename = "Name")]
        pub name: String,
        #[serde(rename = "Signal", default)]
        pub signals: Vec<PinSignal>,
    }

    #[derive(Clone, Debug, Deserialize, PartialEq)]
    pub struct PinSignal {
        #[serde(rename = "Name")]
        pub name: String,
    }

    #[derive(Clone, Debug, Deserialize, PartialEq)]
    pub struct Ip {
        #[serde(rename = "InstanceName")]
        pub instance_name: String,
        #[serde(rename = "Name")]
        pub name: String,
        #[serde(rename = "Version")]
        pub version: String,
    }
}

pub struct Chip {
    flash: u32,
    ram: u32,
    group_idx: usize,
    packages: Vec<stm32_data_serde::chip::Package>,
}

pub struct ChipGroup {
    chip_names: Vec<String>,
    xml: xml::Mcu,
    pub ips: HashMap<String, xml::Ip>,
    pins: HashMap<String, xml::Pin>,
    family: Option<String>,
    line: Option<String>,
    die: Option<String>,
}

fn chip_name_from_package_name(x: &str) -> String {
    let regexes = [
        (regex!("^(STM32L1....).x([AX])$"), "$1-$2"),
        (regex!("^(STM32G0....).xN$"), "$1"),
        (regex!("^(STM32F412..).xP$"), "$1"),
        (regex!("^(STM32L4....).x[PS]$"), "$1"),
        (regex!("^(STM32WB....).x[AE]$"), "$1"),
        (regex!("^(STM32G0....).xN$"), "$1"),
        (regex!("^(STM32L5....).x[PQ]$"), "$1"),
        (regex!("^(STM32L0....).xS$"), "$1"),
        (regex!("^(STM32H7....).xQ$"), "$1"),
        (regex!("^(STM32U5....).xQ$"), "$1"),
        (regex!("^(STM32H5....).xQ$"), "$1"),
        (regex!("^(STM32WBA....).x$"), "$1"),
        (regex!("^(STM32......).x$"), "$1"),
    ];

    regexes
        .iter()
        .find_map(|(a, b)| {
            a.captures(x).map(|cap| {
                let mut res = String::new();
                cap.expand(b, &mut res);
                res
            })
        })
        .unwrap_or_else(|| panic!("bad name: {x}"))
}

struct PeriMatcher {
    regexes: Vec<(regex::Regex, (&'static str, &'static str, &'static str))>,
    cached: HashMap<String, Option<(&'static str, &'static str, &'static str)>>,
}

impl PeriMatcher {
    fn new() -> Self {
        const PERIMAP: &[(&str, (&str, &str, &str))] = &[
            (".*:USART:sci2_v1_1", ("usart", "v1", "USART")),
            (".*:USART:sci2_v1_2_F1", ("usart", "v1", "USART")),
            (".*:USART:sci2_v1_2", ("usart", "v2", "USART")),
            (".*:USART:sci2_v2_0", ("usart", "v3", "USART")),
            (".*:USART:sci2_v2_1", ("usart", "v3", "USART")),
            (".*:USART:sci2_v2_2", ("usart", "v3", "USART")),
            (".*:USART:sci3_v1_0", ("usart", "v3", "USART")),
            (".*:USART:sci3_v1_1", ("usart", "v3", "USART")),
            (".*:USART:sci3_v1_2", ("usart", "v4", "USART")),
            (".*:USART:sci3_v2_0", ("usart", "v4", "USART")),
            (".*:USART:sci3_v2_1", ("usart", "v4", "USART")),
            (".*:UART:sci2_v1_1", ("usart", "v1", "USART")),
            (".*:UART:sci2_v1_2_F4", ("usart", "v2", "USART")),
            (".*:UART:sci2_v2_1", ("usart", "v3", "USART")),
            (".*:UART:sci2_v3_0", ("usart", "v4", "USART")),
            (".*:UART:sci2_v3_1", ("usart", "v4", "USART")),
            (".*:LPUART:sci3_v1_1", ("usart", "v3", "LPUART")),
            (".*:LPUART:sci3_v1_2", ("usart", "v4", "LPUART")),
            (".*:LPUART:sci3_v1_3", ("usart", "v4", "LPUART")),
            (".*:LPUART:sci3_v1_4", ("usart", "v4", "LPUART")),
            ("STM32[HU]5.*:RNG:.*", ("rng", "v3", "RNG")),
            ("STM32L5.*:RNG:.*", ("rng", "v2", "RNG")),
            ("STM32L4[PQ]5.*:RNG:.*", ("rng", "v2", "RNG")),
            ("STM32WL.*:RNG:.*", ("rng", "v2", "RNG")),
            ("STM32F2.*:RNG:.*", ("rng", "v1", "RNG")),
            ("STM32F4.*:RNG:.*", ("rng", "v1", "RNG")),
            ("STM32F7.*:RNG:.*", ("rng", "v1", "RNG")),
            ("STM32L0.*:RNG:.*", ("rng", "v1", "RNG")),
            ("STM32L4.*:RNG:.*", ("rng", "v1", "RNG")),
            ("STM32H7.*:RNG:.*", ("rng", "v1", "RNG")),
            ("STM32G0.*:RNG:.*", ("rng", "v1", "RNG")),
            ("STM32G4.*:RNG:.*", ("rng", "v1", "RNG")),
            ("STM32WB.*:RNG:.*", ("rng", "v1", "RNG")),
            ("STM32F7.*:AES:.*", ("aes", "f7", "AES")),
            ("STM32F4.*:AES:.*", ("aes", "v1", "AES")),
            ("STM32G0.*:AES:.*", ("aes", "v2", "AES")),
            ("STM32G4.*:AES:.*", ("aes", "v2", "AES")),
            ("STM32L0.*:AES:.*", ("aes", "v1", "AES")),
            ("STM32L1.*:AES:.*", ("aes", "v1", "AES")),
            ("STM32L4.*:AES:.*", ("aes", "v1", "AES")),
            ("STM32L5.*:AES:.*", ("aes", "v2", "AES")),
            ("STM32WL5.*:AES:.*", ("aes", "v2", "AES")),
            ("STM32WLE.*:AES:.*", ("aes", "v2", "AES")),
            ("STM32U5.*:AES:.*", ("aes", "v3a", "AES")),
            ("STM32(H5|WBA).*:AES:.*", ("aes", "v3b", "AES")),
            (".*:SPI:spi2_v1_4", ("spi", "f1", "SPI")),
            (".*:SPI:spi2s1_v2_1", ("spi", "v1", "SPI")),
            (".*:SPI:spi2s1_v2_2", ("spi", "v1", "SPI")),
            (".*:SPI:spi2s1_v2_3", ("spi", "v1", "SPI")),
            (".*:SPI:spi2s1_v2_4", ("spi", "v1", "SPI")),
            (".*:SPI:spi2s1_v3_0", ("spi", "v2", "SPI")),
            (".*:SPI:spi2s1_v3_2", ("spi", "v2", "SPI")),
            (".*:SPI:spi2s1_v3_3", ("spi", "v2", "SPI")),
            (".*:SPI:spi2s1_v3_5", ("spi", "v2", "SPI")),
            (".*:SUBGHZSPI:.*", ("spi", "v2", "SPI")),
            (".*:SPI:spi2s1_v3_1", ("spi", "v2", "SPI")),
            (".*:SPI:spi2s2_v1_1", ("spi", "v3", "SPI")),
            (".*:SPI:spi2s2_v1_0", ("spi", "v3", "SPI")),
            (".*:SPI:spi2s3_v2_1", ("spi", "v4", "SPI")),
            (".*:SPI:spi2s3_v1_1", ("spi", "v5", "SPI")),
            (".*:SPI:spi2s3_v2_0", ("spi", "v5", "SPI")),
            (".*:FMAC:matrix1_v1_0", ("fmac", "v1", "FMAC")),
            (".*:I2C:i2c1_v1_5", ("i2c", "v1", "I2C")),
            (".*:I2C:i2c2_v1_1", ("i2c", "v2", "I2C")),
            (".*:I2C:F0-i2c2_v1_1", ("i2c", "v2", "I2C")),
            (".*:I2C:i2c2_v1_1F7", ("i2c", "v2", "I2C")),
            (".*:I2C:i2c2_v1_1U5", ("i2c", "v2", "I2C")),
            ("STM32F10[1357].*:DAC:dacif_v1_1F1", ("dac", "v1", "DAC")), // Original F1 are v1
            (".*:DAC:dacif_v1_1F1", ("dac", "v2", "DAC")),
            (".*:DAC:F0dacif_v1_1", ("dac", "v2", "DAC")),
            (".*:DAC:F3_dacif_v1_1", ("dac", "v2", "DAC")),
            (".*:DAC:dacif_v1_1", ("dac", "v2", "DAC")),
            (".*:DAC:dacif_v1_2", ("dac", "v2", "DAC")),
            ("STM32L4[1-9A].*:DAC:dacif_v2_0", ("dac", "v3", "DAC")), // L4 non-plus are v3
            (".*:DAC:dacif_v2_0", ("dac", "v5", "DAC")),
            (".*:DAC:dacif_v2_0_U5", ("dac", "v6", "DAC")),
            (".*:DAC:dacif_v3_0", ("dac", "v4", "DAC")),
            (".*:DAC:WL_dacif_v3_0", ("dac", "v4", "DAC")),
            (".*:DAC:G4_dacif_v4_0", ("dac", "v7", "DAC")),
            (".*:DAC:dacif_v5_0", ("dac", "v6", "DAC")),
            (".*:ADC:aditf_v2_5F1", ("adc", "f1", "ADC")),
            (".*:ADC:aditf5_v1_1", ("adc", "f3", "ADC")),
            (".*:ADC:aditf_v2_5", ("adc", "f3_v2", "ADC")),
            (".*:ADC:aditf3_v1_1", ("adc", "f3_v1_1", "ADC")),
            (".*:ADC:aditf4_v1_1", ("adc", "v1", "ADC")),
            (".*:ADC:aditf4_v2_0", ("adc", "l0", "ADC")),
            (".*:ADC:aditf2_v1_1", ("adc", "v2", "ADC")),
            (".*:ADC:aditf5_v2_0", ("adc", "v3", "ADC")),
            (".*:ADC:aditf5_v2_2", ("adc", "v3", "ADC")),
            (".*:ADC:aditf5_v3_0", ("adc", "v4", "ADC")),
            (".*:ADC:aditf5_v3_0_H5", ("adc", "h5", "ADC")),
            (".*:ADC:aditf5_v3_1", ("adc", "v4", "ADC")),
            ("STM32WL5.*:ADC:.*", ("adc", "g0", "ADC")),
            ("STM32WLE.*:ADC:.*", ("adc", "g0", "ADC")),
            ("STM32G0.*:ADC:.*", ("adc", "g0", "ADC")),
            ("STM32G0.*:ADC_COMMON:.*", ("adccommon", "v3", "ADC_COMMON")),
            ("STM32G4.*:ADC:.*", ("adc", "v4", "ADC")),
            ("STM32G4.*:ADC_COMMON:.*", ("adccommon", "v4", "ADC_COMMON")),
            (".*:ADC_COMMON:aditf2_v1_1", ("adccommon", "v2", "ADC_COMMON")),
            (".*:ADC_COMMON:aditf5_v2_0", ("adccommon", "v3", "ADC_COMMON")),
            (".*:ADC_COMMON:aditf5_v2_2", ("adccommon", "v3", "ADC_COMMON")),
            (".*:ADC_COMMON:aditf4_v3_0_WL", ("adccommon", "v3", "ADC_COMMON")),
            (".*:ADC_COMMON:aditf5_v1_1", ("adccommon", "f3", "ADC_COMMON")),
            (".*:ADC3_COMMON:aditf5_v1_1", ("adccommon", "f3", "ADC_COMMON")),
            (
                "STM32H50.*:ADC_COMMON:aditf5_v3_0_H5",
                ("adccommon", "h50", "ADC_COMMON"),
            ),
            ("STM32H5.*:ADC_COMMON:aditf5_v3_0_H5", ("adccommon", "h5", "ADC_COMMON")),
            ("STM32H7.*:ADC_COMMON:.*", ("adccommon", "v4", "ADC_COMMON")),
            ("STM32H7.*:ADC3_COMMON:.*", ("adccommon", "v4", "ADC_COMMON")),
            ("STM32G4.*:OPAMP:G4_tsmc90_fastOpamp", ("opamp", "g4", "OPAMP")),
            ("STM32F3.*:OPAMP:tsmc018_ull_opamp_v1_0", ("opamp", "f3", "OPAMP")),
            (".*:DCMI:.*", ("dcmi", "v1", "DCMI")),
            ("STM32C0.*:SYSCFG:.*", ("syscfg", "c0", "SYSCFG")),
            ("STM32F0.*:SYSCFG:.*", ("syscfg", "f0", "SYSCFG")),
            ("STM32F2.*:SYSCFG:.*", ("syscfg", "f2", "SYSCFG")),
            ("STM32F3.*:SYSCFG:.*", ("syscfg", "f3", "SYSCFG")),
            ("STM32F4.*:SYSCFG:.*", ("syscfg", "f4", "SYSCFG")),
            ("STM32F7.*:SYSCFG:.*", ("syscfg", "f7", "SYSCFG")),
            ("STM32L0.*:SYSCFG:.*", ("syscfg", "l0", "SYSCFG")),
            ("STM32L1.*:SYSCFG:.*", ("syscfg", "l1", "SYSCFG")),
            ("STM32L4.*:SYSCFG:.*", ("syscfg", "l4", "SYSCFG")),
            ("STM32L5.*:SYSCFG:.*", ("syscfg", "l5", "SYSCFG")),
            ("STM32G0.*:SYSCFG:.*", ("syscfg", "g0", "SYSCFG")),
            ("STM32G4.*:SYSCFG:.*", ("syscfg", "g4", "SYSCFG")),
            (
                "STM32H7(45|47|55|57|42|43|53|50).*:SYSCFG:.*",
                ("syscfg", "h7od", "SYSCFG"),
            ),
            ("STM32H7.*:SYSCFG:.*", ("syscfg", "h7", "SYSCFG")),
            ("STM32U5.*:SYSCFG:.*", ("syscfg", "u5", "SYSCFG")),
            ("STM32WBA.*:SYSCFG:.*", ("syscfg", "wba", "SYSCFG")),
            ("STM32WB.*:SYSCFG:.*", ("syscfg", "wb", "SYSCFG")),
            ("STM32WL5.*:SYSCFG:.*", ("syscfg", "wl5", "SYSCFG")),
            ("STM32WLE.*:SYSCFG:.*", ("syscfg", "wle", "SYSCFG")),
            ("STM32H50.*:SBS:.*", ("syscfg", "h50", "SYSCFG")),
            ("STM32H5.*:SBS:.*", ("syscfg", "h5", "SYSCFG")),
            (".*:IWDG:iwdg1_v1_1", ("iwdg", "v1", "IWDG")),
            (".*:IWDG:iwdg1_v2_0", ("iwdg", "v2", "IWDG")),
            (".*:IWDG:iwdg1_v3_0", ("iwdg", "v3", "IWDG")),
            (".*:WWDG:wwdg1_v1_0", ("wwdg", "v1", "WWDG")),
            (".*:WWDG:wwdg1_v2_0", ("wwdg", "v2", "WWDG")),
            (".*:JPEG:jpeg1_v1_0", ("jpeg", "v1", "JPEG")),
            (".*:LTDC:lcdtft1_v1_1", ("ltdc", "v1", "LTDC")),
            (".*:MDIOS:mdios1_v1_0", ("mdios", "v1", "MDIOS")),
            (".*:QUADSPI:.*", ("quadspi", "v1", "QUADSPI")),
            ("STM32F1.*:BKP.*", ("bkp", "v1", "BKP")),
            (".*:RTC:rtc1_v1_1", ("rtc", "v1", "RTC")),
            ("STM32F0.*:RTC:rtc2_.*", ("rtc", "v2f0", "RTC")),
            ("STM32F2.*:RTC:rtc2_.*", ("rtc", "v2f2", "RTC")),
            ("STM32F3.*:RTC:rtc2_.*", ("rtc", "v2f3", "RTC")),
            ("STM32F4.*:RTC:rtc2_.*", ("rtc", "v2f4", "RTC")),
            ("STM32F7.*:RTC:rtc2_.*", ("rtc", "v2f7", "RTC")),
            ("STM32H7.*:RTC:rtc2_.*", ("rtc", "v2h7", "RTC")),
            ("STM32L0.*:RTC:rtc2_.*", ("rtc", "v2l0", "RTC")),
            ("STM32L1.*:RTC:rtc2_.*", ("rtc", "v2l1", "RTC")),
            ("STM32L4.*:RTC:rtc2_.*", ("rtc", "v2l4", "RTC")),
            ("STM32L5.*:RTC:rtc2_.*", ("rtc", "v3l5", "RTC")),
            ("STM32WBA.*:RTC:rtc2_.*", ("rtc", "v3u5", "RTC")),
            ("STM32WB.*:RTC:rtc2_.*", ("rtc", "v2wb", "RTC")),
            ("STM32H5.*:RTC:rtc2_.*", ("rtc", "v3u5", "RTC")),
            ("STM32U5.*:RTC:rtc2_.*", ("rtc", "v3u5", "RTC")), // Cube says v2, but it's v3 with security stuff
            (".*:RTC:rtc3_v1_0", ("rtc", "v3", "RTC")),
            (".*:RTC:rtc3_v1_1", ("rtc", "v3", "RTC")),
            (".*:RTC:rtc3_v2_0", ("rtc", "v3", "RTC")),
            (".*:RTC:rtc3_v3_0", ("rtc", "v3", "RTC")),
            (".*:SAI:sai1_v1_0", ("sai", "v1", "SAI")),
            (".*:SAI:sai1_v1_1", ("sai", "v2", "SAI")),
            (".*:SAI:sai1_v1_2", ("sai", "v2", "SAI")),
            (".*:SAI:sai1_v2_0", ("sai", "v1", "SAI")),
            (".*:SAI:sai1_H7", ("sai", "v3", "SAI")),
            (".*:SAI:sai1_v2_1", ("sai", "v4", "SAI")),
            (".*:SDIO:sdmmc_v1_2", ("sdmmc", "v1", "SDMMC")),
            (".*:SDMMC:sdmmc_v1_3", ("sdmmc", "v1", "SDMMC")),
            (".*:SPDIFRX:spdifrx1_v1_0", ("spdifrx", "v1", "SPDIFRX")),
            // # USB
            ("STM32(F1|L1).*:USB:.*", ("usb", "v1", "USB")),
            ("STM32(F1|L1).*:USBRAM:.*", ("usbram", "16x1_512", "USBRAM")),
            ("STM32F30[23].[BC].*:USB:.*", ("usb", "v1", "USB")),
            ("STM32F30[23].[BC].*:USBRAM:.*", ("usbram", "16x1_512", "USBRAM")),
            ("STM32F30[23].[68DE].*:USB:.*", ("usb", "v2", "USB")),
            ("STM32F30[23].[68DE].*:USBRAM:.*", ("usbram", "16x2_1024", "USBRAM")),
            ("STM32F373.*:USB:.*", ("usb", "v1", "USB")),
            ("STM32F373.*:USBRAM:.*", ("usbram", "16x2_512", "USBRAM")),
            ("STM32(F0|L[045]|G4|WB).*:USB:.*", ("usb", "v3", "USB")),
            ("STM32(F0|L[045]|G4|WB).*:USBRAM:.*", ("usbram", "16x2_1024", "USBRAM")),
            ("STM32(G0|H5|U5).*:USB:.*", ("usb", "v4", "USB")),
            ("STM32(G0|H5|U5).*:USBRAM:.*", ("usbram", "32_2048", "USBRAM")),
            // # USB OTG
            (".*:USB_OTG_FS:otgfs1_.*", ("otg", "v1", "OTG")),
            (".*:USB_OTG_HS:otghs1_.*", ("otg", "v1", "OTG")),
            ("STM32C0.*:RCC:.*", ("rcc", "c0", "RCC")),
            ("STM32F030.[46].*:RCC:.*", ("rcc", "f0v1", "RCC")),
            ("STM32F05[128].*:RCC:.*", ("rcc", "f0v1", "RCC")),
            ("STM32F030.8.*:RCC:.*", ("rcc", "f0v2", "RCC")),
            ("STM32F03[128].*:RCC:.*", ("rcc", "f0v2", "RCC")),
            ("STM32F030.C.*:RCC:.*", ("rcc", "f0v3", "RCC")),
            ("STM32F070.[6B].*:RCC:.*", ("rcc", "f0v3", "RCC")),
            ("STM32F0[479][128].*:RCC:.*", ("rcc", "f0v4", "RCC")),
            ("STM32F100.*:RCC:.*", ("rcc", "f100", "RCC")),
            ("STM32F10[123].*:RCC:.*", ("rcc", "f1", "RCC")),
            ("STM32F10[57].*:RCC:.*", ("rcc", "f1cl", "RCC")),
            ("STM32F2.*:RCC:.*", ("rcc", "f2", "RCC")),
            ("STM32F37.*:RCC:.*", ("rcc", "f37", "RCC")),
            ("STM32F30[23].[BC].*:RCC:.*", ("rcc", "f3v1", "RCC")),
            ("STM32F358.C.*:RCC:.*", ("rcc", "f3v1", "RCC")),
            ("STM32F30[23].[DE].*:RCC:.*", ("rcc", "f3v3", "RCC")),
            ("STM32F398.E.*:RCC:.*", ("rcc", "f3v3", "RCC")),
            ("STM32F3.*:RCC:.*", ("rcc", "f3v2", "RCC")),
            ("STM32F410.*:RCC:.*", ("rcc", "f410", "RCC")),
            ("STM32F4.*:RCC:.*", ("rcc", "f4", "RCC")),
            ("STM32F7.*:RCC:.*", ("rcc", "f7", "RCC")),
            ("STM32G0.*:RCC:.*", ("rcc", "g0", "RCC")),
            ("STM32G4.*:RCC:.*", ("rcc", "g4", "RCC")),
            ("STM32H7[AB].*:RCC:.*", ("rcc", "h7ab", "RCC")),
            ("STM32H7(42|43|53|50).*:RCC:.*", ("rcc", "h7rm0433", "RCC")),
            ("STM32H7.*:RCC:.*", ("rcc", "h7", "RCC")),
            ("STM32L0.[23].*:RCC:.*", ("rcc", "l0_v2", "RCC")),
            ("STM32L0.*:RCC:.*", ("rcc", "l0", "RCC")),
            ("STM32L1.*:RCC:.*", ("rcc", "l1", "RCC")),
            ("STM32L4[PQRS].*:RCC:.*", ("rcc", "l4plus", "RCC")),
            ("STM32L4.*:RCC:.*", ("rcc", "l4", "RCC")),
            ("STM32L5.*:RCC:.*", ("rcc", "l5", "RCC")),
            ("STM32U5.*:RCC:.*", ("rcc", "u5", "RCC")),
            ("STM32H50.*:RCC:.*", ("rcc", "h50", "RCC")),
            ("STM32H5.*:RCC:.*", ("rcc", "h5", "RCC")),
            ("STM32WBA.*:RCC:.*", ("rcc", "wba", "RCC")),
            ("STM32WB.*:RCC:.*", ("rcc", "wb", "RCC")),
            ("STM32WL5.*:RCC:.*", ("rcc", "wl5", "RCC")),
            ("STM32WLE.*:RCC:.*", ("rcc", "wle", "RCC")),
            ("STM32F1.*:SPI[1234]:.*", ("spi", "f1", "SPI")),
            ("STM32F3.*:SPI[1234]:.*", ("spi", "v2", "SPI")),
            ("STM32F1.*:AFIO:.*", ("afio", "f1", "AFIO")),
            ("STM32WBA.*:EXTI:.*", ("exti", "l5", "EXTI")),
            ("STM32L5.*:EXTI:.*", ("exti", "l5", "EXTI")),
            ("STM32C0.*:EXTI:.*", ("exti", "c0", "EXTI")),
            ("STM32G0.*:EXTI:.*", ("exti", "g0", "EXTI")),
            ("STM32H7.*:EXTI:.*", ("exti", "h7", "EXTI")),
            ("STM32U5.*:EXTI:.*", ("exti", "u5", "EXTI")),
            ("STM32WB.*:EXTI:.*", ("exti", "w", "EXTI")),
            ("STM32WL5.*:EXTI:.*", ("exti", "w", "EXTI")),
            ("STM32WLE.*:EXTI:.*", ("exti", "wle", "EXTI")),
            ("STM32H50.*:EXTI:.*", ("exti", "h50", "EXTI")),
            ("STM32H5.*:EXTI:.*", ("exti", "h5", "EXTI")),
            (".*:EXTI:.*", ("exti", "v1", "EXTI")),
            ("STM32F0.*:CRS:.*", ("crs", "v1", "CRS")),
            ("STM32L0.*:CRS:.*", ("crs", "v1", "CRS")),
            ("STM32L4.*:CRS:.*", ("crs", "v1", "CRS")),
            ("STM32L5.*:CRS:.*", ("crs", "v1", "CRS")),
            ("STM32G0.*:CRS:.*", ("crs", "v1", "CRS")),
            ("STM32G4.*:CRS:.*", ("crs", "v1", "CRS")),
            ("STM32U5.*:CRS:.*", ("crs", "v1", "CRS")),
            ("STM32H5.*:CRS:.*", ("crs", "v1", "CRS")),
            ("STM32H7.*:CRS:.*", ("crs", "v1", "CRS")),
            ("STM32WB.*:CRS:.*", ("crs", "v1", "CRS")),
            (".*SDMMC:sdmmc2_v1_0", ("sdmmc", "v2", "SDMMC")),
            (".*SDMMC:sdmmc2_v2_1", ("sdmmc", "v2", "SDMMC")),
            ("STM32C0.*:PWR:.*", ("pwr", "c0", "PWR")),
            ("STM32G0.*:PWR:.*", ("pwr", "g0", "PWR")),
            ("STM32G4.*:PWR:.*", ("pwr", "g4", "PWR")),
            ("STM32H7(45|47|55|57).*:PWR:.*", ("pwr", "h7rm0399", "PWR")),
            ("STM32H7(42|43|53|50).*:PWR:.*", ("pwr", "h7rm0433", "PWR")),
            ("STM32H7(23|25|33|35|30).*:PWR:.*", ("pwr", "h7rm0468", "PWR")),
            ("STM32H7(A3|B0|B3).*:PWR:.*", ("pwr", "h7rm0455", "PWR")),
            ("STM32F0.0.*:PWR:.*", ("pwr", "f0x0", "PWR")),
            ("STM32F0.*:PWR:.*", ("pwr", "f0", "PWR")),
            ("STM32F1.*:PWR:.*", ("pwr", "f1", "PWR")),
            ("STM32F2.*:PWR:.*", ("pwr", "f2", "PWR")),
            ("STM32F3.*:PWR:.*", ("pwr", "f3", "PWR")),
            ("STM32F4.*:PWR:.*", ("pwr", "f4", "PWR")),
            ("STM32F7.*:PWR:.*", ("pwr", "f7", "PWR")),
            ("STM32L0.*:PWR:.*", ("pwr", "l0", "PWR")),
            ("STM32L1.*:PWR:.*", ("pwr", "l1", "PWR")),
            ("STM32L4.*:PWR:.*", ("pwr", "l4", "PWR")),
            ("STM32L5.*:PWR:.*", ("pwr", "l5", "PWR")),
            ("STM32U5.*:PWR:.*", ("pwr", "u5", "PWR")),
            ("STM32WL.*:PWR:.*", ("pwr", "wl5", "PWR")),
            ("STM32WBA.*:PWR:.*", ("pwr", "wba", "PWR")),
            ("STM32WB[35]5.*:PWR:.*", ("pwr", "wb55", "PWR")),
            ("STM32WB.*:PWR:.*", ("pwr", "wb", "PWR")),
            ("STM32H50.*:PWR:.*", ("pwr", "h50", "PWR")),
            ("STM32H5.*:PWR:.*", ("pwr", "h5", "PWR")),
            ("STM32H7(A3|B3|B0).*:FLASH:.*", ("flash", "h7ab", "FLASH")),
            ("STM32H7.*:FLASH:.*", ("flash", "h7", "FLASH")),
            ("STM32F0.*:FLASH:.*", ("flash", "f0", "FLASH")),
            ("STM32F1.*:FLASH:.*", ("flash", "f1", "FLASH")),
            ("STM32F2.*:FLASH:.*", ("flash", "f2", "FLASH")),
            ("STM32F3.*:FLASH:.*", ("flash", "f3", "FLASH")),
            ("STM32F4.*:FLASH:.*", ("flash", "f4", "FLASH")),
            ("STM32F7.*:FLASH:.*", ("flash", "f7", "FLASH")),
            ("STM32L0.*:FLASH:.*", ("flash", "l0", "FLASH")),
            ("STM32L1.*:FLASH:.*", ("flash", "l1", "FLASH")),
            ("STM32L4.*:FLASH:.*", ("flash", "l4", "FLASH")),
            ("STM32L5.*:FLASH:.*", ("flash", "l5", "FLASH")),
            ("STM32U5.*:FLASH:.*", ("flash", "u5", "FLASH")),
            ("STM32WBA.*:FLASH:.*", ("flash", "wba", "FLASH")),
            ("STM32WB.*:FLASH:.*", ("flash", "wb", "FLASH")),
            ("STM32WL.*:FLASH:.*", ("flash", "wl", "FLASH")),
            ("STM32C0.*:FLASH:.*", ("flash", "c0", "FLASH")),
            ("STM32G0.*:FLASH:.*", ("flash", "g0", "FLASH")),
            ("STM32G4.*:FLASH:.*", ("flash", "g4", "FLASH")),
            ("STM32H50.*:FLASH:.*", ("flash", "h50", "FLASH")),
            ("STM32H5.*:FLASH:.*", ("flash", "h5", "FLASH")),
            ("STM32F107.*:ETH:.*", ("eth", "v1a", "ETH")),
            ("STM32F[24].*:ETH:.*", ("eth", "v1b", "ETH")),
            ("STM32F7.*:ETH:.*", ("eth", "v1c", "ETH")),
            ("STM32H.*:ETH:.*", ("eth", "v2", "ETH")),
            ("STM32F4[23][79].*:FMC:.*", ("fmc", "v1x3", "FMC")),
            ("STM32F446.*:FMC:.*", ("fmc", "v2x1", "FMC")),
            ("STM32F469.*:FMC:.*", ("fmc", "v2x1", "FMC")),
            ("STM32F7.*:FMC:.*", ("fmc", "v2x1", "FMC")),
            ("STM32H7.*:FMC:.*", ("fmc", "v3x1", "FMC")),
            ("STM32F100.*:FSMC:.*", ("fsmc", "v1x0", "FSMC")),
            ("STM32F10[12357].*:FSMC:.*", ("fsmc", "v1x3", "FSMC")),
            ("STM32F2.*:FSMC:.*", ("fsmc", "v1x3", "FSMC")),
            ("STM32F3.*:FSMC:.*", ("fsmc", "v2x3", "FSMC")),
            ("STM32F412.*:FSMC:.*", ("fsmc", "v1x0", "FSMC")),
            ("STM32F4[12]3.*:FSMC:.*", ("fsmc", "v1x0", "FSMC")),
            ("STM32F4[01]5.*:FSMC:.*", ("fsmc", "v1x3", "FSMC")),
            ("STM32F4[01]7.*:FSMC:.*", ("fsmc", "v1x3", "FSMC")),
            ("STM32L1.*:FSMC:.*", ("fsmc", "v1x0", "FSMC")),
            ("STM32L4.*:FSMC:.*", ("fsmc", "v3x1", "FSMC")),
            ("STM32G4.*:FSMC:.*", ("fsmc", "v4x1", "FSMC")),
            ("STM32L5.*:FSMC:.*", ("fsmc", "v4x1", "FSMC")),
            ("STM32U5.*:FSMC:.*", ("fsmc", "v5x1", "FSMC")),
            //// TIM mapping starts here ////
            //
            // Note:
            // AN4013 for the full tables of TIMs
            // AN4013 Rev: 10, Date: 12-Jan-2023
            //
            //
            // AN4013 Table 2: STM32Fx serials
            // Override for STM32Fx serials
            ("STM32F1.*:TIM(2|5):.*", ("timer", "v1", "TIM_GP16")),
            // Normal STM32Fx serials
            ("STM32F.*:TIM(1|8|20):.*", ("timer", "v1", "TIM_ADV")),
            ("STM32F.*:TIM(2|5):.*", ("timer", "v1", "TIM_GP32")),
            ("STM32F.*:TIM(3|4|19):.*", ("timer", "v1", "TIM_GP16")),
            ("STM32F.*:TIM(6|7|18):.*", ("timer", "v1", "TIM_BASIC")),
            ("STM32F.*:TIM(10|11|13|14):.*", ("timer", "v1", "TIM_1CH")),
            ("STM32F.*:TIM(9|12):.*", ("timer", "v1", "TIM_2CH")),
            ("STM32F.*:TIM15:.*", ("timer", "v1", "TIM_2CH_CMP")),
            ("STM32F.*:TIM(16|17):.*", ("timer", "v1", "TIM_1CH_CMP")),
            ("STM32F.*:LPTIM1:.*", ("lptim", "v1", "LPTIM")),
            ("STM32F.*:HRTIM:.*", ("hrtim", "v1", "HRTIM")),
            // AN4013 Table 3: STM32Lx serials
            // Override for STM32L0 serial
            ("STM32L0.*:TIM(2|3):.*", ("timer", "l0", "TIM_GP16")),
            ("STM32L0.*:TIM(6|7):.*", ("timer", "l0", "TIM_BASIC")),
            ("STM32L0.*:TIM(21|22):.*", ("timer", "l0", "TIM_2CH")),
            // Override for STM32L1 serials
            ("STM32L1.*:TIM2:.*", ("timer", "v1", "TIM_GP16")),
            // Normal STM32Lx serials
            ("STM32L.*:TIM(1|8):.*", ("timer", "v1", "TIM_ADV")),
            ("STM32L.*:TIM(2|5):.*", ("timer", "v1", "TIM_GP32")),
            ("STM32L.*:TIM(3|4):.*", ("timer", "v1", "TIM_GP16")),
            ("STM32L.*:TIM(6|7):.*", ("timer", "v1", "TIM_BASIC")),
            ("STM32L.*:TIM(10|11):.*", ("timer", "v1", "TIM_1CH")),
            ("STM32L.*:TIM(9|21|22):.*", ("timer", "v1", "TIM_2CH")),
            ("STM32L.*:TIM15:.*", ("timer", "v1", "TIM_2CH_CMP")),
            ("STM32L.*:TIM(16|17):.*", ("timer", "v1", "TIM_1CH_CMP")),
            ("STM32L.*:LPTIM(1|2|3):.*", ("lptim", "v1", "LPTIM")),
            // AN4013 Table 4: STM32Gx/Hx/Ux/Wx (and Cx) serials
            // timer_v2 for STM32Gx/Hx/Ux/Wx (and Cx) serials
            ("STM32U5.*:TIM(3|4):.*", ("timer", "v2", "TIM_GP32")),
            ("STM32(G4|H5|U5|WBA).*:TIM(1|8|20):.*", ("timer", "v2", "TIM_ADV")),
            ("STM32(G4|H5|U5|WBA).*:TIM(2|5|23|24):.*", ("timer", "v2", "TIM_GP32")),
            ("STM32(G4|H5|U5|WBA).*:TIM(3|4):.*", ("timer", "v2", "TIM_GP16")),
            ("STM32(G4|H5|U5|WBA).*:TIM(6|7):.*", ("timer", "v2", "TIM_BASIC")),
            ("STM32(G4|H5|U5|WBA).*:TIM(13|14):.*", ("timer", "v2", "TIM_1CH")),
            ("STM32(G4|H5|U5|WBA).*:TIM12:.*", ("timer", "v2", "TIM_2CH")),
            ("STM32(G4|H5|U5|WBA).*:TIM15:.*", ("timer", "v2", "TIM_2CH_CMP")),
            ("STM32(G4|H5|U5|WBA).*:TIM(16|17):.*", ("timer", "v2", "TIM_1CH_CMP")),
            ("STM32G4.*:HRTIM1:.*", ("hrtim", "v2", "HRTIM")),
            // timer_v1 for STM32Gx/Hx/Ux/Wx (and Cx) serials
            ("STM32(C|G0|H7|WB|WL).*:TIM(1|8|20):.*", ("timer", "v1", "TIM_ADV")),
            ("STM32(C|G0|H7|WB|WL).*:TIM(2|5|23|24):.*", ("timer", "v1", "TIM_GP32")),
            ("STM32(C|G0|H7|WB|WL).*:TIM(3|4):.*", ("timer", "v1", "TIM_GP16")),
            ("STM32(C|G0|H7|WB|WL).*:TIM(6|7):.*", ("timer", "v1", "TIM_BASIC")),
            ("STM32(C|G0|H7|WB|WL).*:TIM(13|14):.*", ("timer", "v1", "TIM_1CH")),
            ("STM32(C|G0|H7|WB|WL).*:TIM12:.*", ("timer", "v1", "TIM_2CH")),
            ("STM32(C|G0|H7|WB|WL).*:TIM15:.*", ("timer", "v1", "TIM_2CH_CMP")),
            ("STM32(C|G0|H7|WB|WL).*:TIM(16|17):.*", ("timer", "v1", "TIM_1CH_CMP")),
            ("STM32[CGHUW].*:LPTIM[1-6]:.*", ("lptim", "v1", "LPTIM")),
            ("STM32[CGHUW].*:HRTIM1?:.*", ("hrtim", "v1", "HRTIM")),
            //
            //// TIM mapping ends here ////
            ("STM32F0.*:DBGMCU:.*", ("dbgmcu", "f0", "DBGMCU")),
            ("STM32F1.*:DBGMCU:.*", ("dbgmcu", "f1", "DBGMCU")),
            ("STM32F2.*:DBGMCU:.*", ("dbgmcu", "f2", "DBGMCU")),
            ("STM32F3.*:DBGMCU:.*", ("dbgmcu", "f3", "DBGMCU")),
            ("STM32F4.*:DBGMCU:.*", ("dbgmcu", "f4", "DBGMCU")),
            ("STM32F7.*:DBGMCU:.*", ("dbgmcu", "f7", "DBGMCU")),
            ("STM32C0.*:DBGMCU:.*", ("dbgmcu", "c0", "DBGMCU")),
            ("STM32G0.*:DBGMCU:.*", ("dbgmcu", "g0", "DBGMCU")),
            ("STM32G4.*:DBGMCU:.*", ("dbgmcu", "g4", "DBGMCU")),
            ("STM32H5.*:DBGMCU:.*", ("dbgmcu", "h5", "DBGMCU")),
            ("STM32H7.*:DBGMCU:.*", ("dbgmcu", "h7", "DBGMCU")),
            ("STM32L0.*:DBGMCU:.*", ("dbgmcu", "l0", "DBGMCU")),
            ("STM32L1.*:DBGMCU:.*", ("dbgmcu", "l1", "DBGMCU")),
            ("STM32L4.*:DBGMCU:.*", ("dbgmcu", "l4", "DBGMCU")),
            ("STM32L5.*:DBGMCU:.*", ("dbgmcu", "l5", "DBGMCU")),
            ("STM32U5.*:DBGMCU:.*", ("dbgmcu", "u5", "DBGMCU")),
            ("STM32WBA.*:DBGMCU:.*", ("dbgmcu", "wba", "DBGMCU")),
            ("STM32WB.*:DBGMCU:.*", ("dbgmcu", "wb", "DBGMCU")),
            ("STM32WL.*:DBGMCU:.*", ("dbgmcu", "wl", "DBGMCU")),
            ("STM32F1.*:GPIO.*", ("gpio", "v1", "GPIO")),
            (".*:GPIO.*", ("gpio", "v2", "GPIO")),
            (".*:IPCC:v1_0", ("ipcc", "v1", "IPCC")),
            (".*:DMAMUX.*", ("dmamux", "v1", "DMAMUX")),
            (r".*:GPDMA\d?:.*", ("gpdma", "v1", "GPDMA")),
            (r".*:BDMA\d?:.*", ("bdma", "v1", "DMA")),
            ("STM32H7.*:DMA2D:DMA2D:dma2d1_v1_0", ("dma2d", "v2", "DMA2D")),
            (".*:DMA2D:dma2d1_v1_0", ("dma2d", "v1", "DMA2D")),
            ("STM32L4[PQRS].*:DMA.*", ("bdma", "v1", "DMA")), // L4+
            ("STM32L[04].*:DMA.*", ("bdma", "v2", "DMA")),    // L0, L4 non-plus (since plus is handled above)
            ("STM32F030.C.*:DMA.*", ("bdma", "v2", "DMA")),   // Weird F0
            ("STM32F09.*:DMA.*", ("bdma", "v2", "DMA")),      // Weird F0
            ("STM32F[247].*:DMA.*", ("dma", "v2", "DMA")),
            ("STM32H7.*:DMA.*", ("dma", "v1", "DMA")),
            (".*:DMA.*", ("bdma", "v1", "DMA")),
            (".*:CAN:bxcan1_v1_1.*", ("can", "bxcan", "CAN")),
            ("STM32H7.*:FDCAN:fdcan1_v1_[01].*", ("can", "fdcan_h7", "FDCAN")),
            (".*:FDCAN:fdcan1_v1_[01].*", ("can", "fdcan_v1", "FDCAN")),
            ("STM32H7.*:FDCANRAM.*", ("fdcanram", "h7", "FDCANRAM")),
            (".*:FDCANRAM.*", ("fdcanram", "v1", "FDCANRAM")),
            ("STM32F[124].*:CRC:.*", ("crc", "v1", "CRC")),
            ("STM32L1.*:CRC:.*", ("crc", "v1", "CRC")),
            ("STM32F0.*:CRC:.*", ("crc", "v2", "CRC")),
            ("STM32F[37].*:CRC:.*", ("crc", "v3", "CRC")),
            ("STM32G[04].*:CRC:.*", ("crc", "v3", "CRC")),
            ("STM32H[57].*:CRC:.*", ("crc", "v3", "CRC")),
            ("STM32L[045].*:CRC:.*", ("crc", "v3", "CRC")),
            ("STM32W[BL].*:CRC:.*", ("crc", "v3", "CRC")),
            ("STM32C[0].*:CRC:.*", ("crc", "v3", "CRC")),
            ("STM32U[5].*:CRC:.*", ("crc", "v3", "CRC")),
            (".*:LCD:lcdc1_v1.0.*", ("lcd", "v1", "LCD")),
            (".*:LCD:lcdc1_v1.2.*", ("lcd", "v2", "LCD")),
            (".*:LCD:lcdc1_v1.3.*", ("lcd", "v2", "LCD")),
            (".*:UID:.*", ("uid", "v1", "UID")),
            (".*:UCPD:.*", ("ucpd", "v1", "UCPD")),
            ("STM32G0.*:TAMP:.*", ("tamp", "g0", "TAMP")),
            ("STM32G4.*:TAMP:.*", ("tamp", "g4", "TAMP")),
            ("STM32H5.*:TAMP:.*", ("tamp", "h5", "TAMP")),
            ("STM32L5.*:TAMP:.*", ("tamp", "l5", "TAMP")),
            ("STM32U5.*:TAMP:.*", ("tamp", "u5", "TAMP")),
            ("STM32WL.*:TAMP:.*", ("tamp", "wl", "TAMP")),
            (".*:OCTOSPIM:OCTOSPIM:.*", ("octospim", "v1", "OCTOSPIM")),
            // it's actually STM32L4+, not STM32L4
            (
                "STM32L4.*:OCTOSPI[12]:OCTOSPI:octospi_v1_0.*",
                ("octospi", "v1", "OCTOSPI"),
            ),
            (
                "STM32H7.*:OCTOSPI[12]:OCTOSPI:octospi_v2_1H7AB.*",
                ("octospi", "v1", "OCTOSPI"),
            ),
            (
                "STM32U5[34].*:OCTOSPI[12]:OCTOSPI:octospi_v1_0L5.*",
                ("octospi", "v1", "OCTOSPI"),
            ),
            (
                "STM32U5[AFG789].*:OCTOSPI[12]:OCTOSPI:octospi1_v3_0.*",
                ("octospi", "v1", "OCTOSPI"),
            ),
            (
                "STM32L5.*:OCTOSPI[12]:OCTOSPI:octospi_v1_0L5.*",
                ("octospi", "v2", "OCTOSPI"),
            ),
            (
                "STM32H5.*:OCTOSPI[12]:OCTOSPI:octospi1_v5_1.*",
                ("octospi", "v2", "OCTOSPI"),
            ),
            ("STM32L4.*:GFXMMU:.*", ("gfxmmu", "v1", "GFXMMU")),
            ("STM32U5.*:GFXMMU:.*", ("gfxmmu", "v2", "GFXMMU")),
            ("STM32U5.*:ICACHE:.*", ("icache", "v1_3crr", "ICACHE")),
            ("STM32H50.*:ICACHE:.*", ("icache", "v1_0crr", "ICACHE")),
            ("STM32(L5|H5[67]|WBA).*:ICACHE:.*", ("icache", "v1_4crr", "ICACHE")),
            (".*:CORDIC:.*", ("cordic", "v1", "CORDIC")),
            ("STM32F0x[128].*:TSC:.*", ("tsc", "v1", "TSC")),
            ("STM32F3[07][123].*:TSC:.*", ("tsc", "v1", "TSC")),
            ("STM32WB55.*:TSC:.*", ("tsc", "v2", "TSC")),
            ("STM32L[045].*:TSC:.*", ("tsc", "v3", "TSC")),
            ("STM32U5.*:TSC:.*", ("tsc", "v3", "TSC")),
            ("*:VREFINTCAL:.*", ("vrefintcal", "v1", "VREFINTCAL")),
            ("STM32U5.*:ADF[12]:.*", ("adf", "v1", "ADF")),
            (".*:HASH:hash1_v1_0", ("hash", "v1", "HASH")),
            (".*:HASH:hash1_v2_0", ("hash", "v2", "HASH")),
            ("STM32U5.*:HASH:.*", ("hash", "v4", "HASH")),
            ("STM32WBA.*:HASH:.*", ("hash", "v4", "HASH")),
            (".*:HASH:hash1_v2_2", ("hash", "v2", "HASH")),
            (".*:HASH:hash1_v4_0", ("hash", "v3", "HASH")),
            (".*:CRYP:cryp1_v1_0.*", ("cryp", "v1", "CRYP")),
            (".*:CRYP:cryp1_v2_0.*", ("cryp", "v2", "CRYP")),
            (".*:CRYP:cryp1_v2_2.*", ("cryp", "v2", "CRYP")),
            ("STM32G0.1.*:.*:COMP:.*", ("comp", "v1", "COMP")),
            ("STM32G4.*:.*:COMP:.*", ("comp", "v2", "COMP")),
            ("STM32WL.*:.*:COMP:.*", ("comp", "v3", "COMP")),
            (r".*:.*:DCACHE:.*", ("dcache", "v1", "DCACHE")),
            (".*:.*:PSSI:.*", ("pssi", "v1", "PSSI")),
            (".*:.*:DTS:.*", ("dts", "v1", "DTS")),
            // HDMI_CEC for F1
            (".*:HDMI_CEC:hdmi_cec_v1_1", ("cec", "v1", "CEC")),
            // HDMI_CEC for others
            (".*:HDMI_CEC:hdmi_cec_v2_0", ("cec", "v2", "CEC")),
            ("STM32(L5|L4|G0|WB|WL).*:VREFBUF:.*", ("vrefbuf", "v1", "VREFBUF")),
            ("STM32(H7|U5).*:VREFBUF:.*", ("vrefbuf", "v2a1", "VREFBUF")),
            ("STM32H5.*:VREFBUF:.*", ("vrefbuf", "v2a2", "VREFBUF")),
            ("STM32G4.*:VREFBUF:.*", ("vrefbuf", "v2b", "VREFBUF")),
        ];

        Self {
            regexes: PERIMAP
                .iter()
                .map(|(a, b)| (regex::Regex::new(&format!("^{a}$")).unwrap(), *b))
                .collect(),
            cached: HashMap::new(),
        }
    }

    fn match_peri(&mut self, peri: &str) -> Option<(&'static str, &'static str, &'static str)> {
        *self
            .cached
            .entry(peri.to_string())
            .or_insert_with(|| self.regexes.iter().find(|(r, _block)| r.is_match(peri)).map(|x| x.1))
    }
}

fn corename(d: &str) -> String {
    let m = regex!(r".*Cortex-M(\d+)(\+?)\s*(.*)").captures(d).unwrap();
    let cm = m.get(1).unwrap().as_str();
    let p = if m.get(2).unwrap().as_str() == "+" { "p" } else { "" };
    let s = if m.get(3).unwrap().as_str() == "secure" {
        "s"
    } else {
        ""
    };
    format!("cm{cm}{p}{s}")
}

fn merge_periph_pins_info(
    chip_name: &str,
    periph_name: &str,
    core_pins: &mut [stm32_data_serde::chip::core::peripheral::Pin],
    af_pins: &[stm32_data_serde::chip::core::peripheral::Pin],
) {
    if chip_name.contains("STM32F1") {
        // TODO: actually handle the F1 AFIO information when it will be extracted
        return;
    }

    // covert to hashmap
    let af_pins: HashMap<(&str, &str), Option<u8>> = af_pins
        .iter()
        .map(|v| ((v.pin.as_str(), v.signal.as_str()), v.af))
        .collect();

    for pin in &mut core_pins[..] {
        let af = af_pins.get(&(&pin.pin, &pin.signal)).copied().flatten();

        // try to look for a signal with another name
        let af = af.or_else(|| {
            if pin.signal == "CTS" {
                // for some godforsaken reason UART4's and UART5's CTS are called CTS_NSS in the GPIO xml
                // so try to match with these
                af_pins.get(&(pin.pin.as_str(), "CTS_NSS")).copied().flatten()
            } else if chip_name.starts_with("STM32F0") && periph_name == "I2C1" {
                // it appears that for __some__ STM32 MCUs there is no AFIO specified in GPIO file
                // (notably - STM32F030C6 with it's I2C1 on PF6 and PF7)
                // but the peripheral can actually be mapped to different pins
                // this breaks embassy's model, so we pretend that it's AF 0
                // Reference Manual states that there's no GPIOF_AFR register
                // but according to Cube-generated core it's OK to write to AFIO reg, it seems to be ignored
                // TODO: are there any more signals that have this "feature"
                Some(0)
            } else {
                None
            }
        });

        if let Some(af) = af {
            pin.af = Some(af);
        }
    }

    // apply some renames
    if chip_name.starts_with("STM32C0") || chip_name.starts_with("STM32G0") {
        for pin in &mut core_pins[..] {
            if pin.signal == "MCO" {
                pin.signal = "MCO_1".to_string()
            }
        }
    }
}

pub fn parse_groups() -> Result<(HashMap<String, Chip>, Vec<ChipGroup>), anyhow::Error> {
    // XMLs group together chips that are identical except flash/ram size.
    // For example STM32L471Z(E-G)Jx.xml is STM32L471ZEJx, STM32L471ZGJx.
    // However they do NOT group together identical chips with different package.

    // We want exactly the opposite: group all packages of a chip together, but
    // NOT group equal-except-memory-size chips together. Yay.

    // We first read all XMLs, and fold together all packages. We don't expand
    // flash/ram sizes yet, we want to do it as late as possible to avoid duplicate
    // work so that generation is faster.

    let mut chips = HashMap::<String, Chip>::new();
    let mut chip_groups = Vec::new();

    let mut files: Vec<_> = glob::glob("sources/cubedb/mcu/STM32*.xml")?
        .map(Result::unwrap)
        .collect();
    files.sort();

    for f in files {
        parse_group(f, &mut chips, &mut chip_groups)?;
    }

    for (chip_name, chip) in &chips {
        chip_groups[chip.group_idx].chip_names.push(chip_name.clone());
    }
    Ok((chips, chip_groups))
}

static NOPELIST: &[&str] = &[
    // Not supported, not planned unless someone wants to do it.
    "STM32MP",
    // Does not exist in ST website. No datasheet, no RM.
    "STM32GBK",
    "STM32L485",
    "STM32U5F",
    "STM32U5G",
    // STM32WxM modules. These are based on a chip that's supported on its own,
    // not sure why we want a separate target for it.
    "STM32WL5M",
    "STM32WB1M",
    "STM32WB3M",
    "STM32WB5M",
];

fn parse_group(
    f: std::path::PathBuf,
    chips: &mut HashMap<String, Chip>,
    chip_groups: &mut Vec<ChipGroup>,
) -> anyhow::Result<()> {
    let ff = f.file_name().unwrap().to_string_lossy();

    for nope in NOPELIST {
        if ff.contains(nope) {
            return Ok(());
        }
    }

    let parsed: xml::Mcu = quick_xml::de::from_str(&std::fs::read_to_string(f)?)?;

    let package_names = {
        let name = &parsed.ref_name;
        if !name.contains('(') {
            vec![name.to_string()]
        } else {
            let (prefix, suffix) = name.split_once('(').unwrap();
            let (letters, suffix) = suffix.split_once(')').unwrap();
            letters.split('-').map(|x| format!("{prefix}{x}{suffix}")).collect()
        }
    };

    let package_rams = {
        if parsed.rams.len() == 1 {
            vec![parsed.rams[0]; package_names.len()]
        } else {
            parsed.rams.clone()
        }
    };
    let package_flashes = {
        if parsed.flashs.len() == 1 {
            vec![parsed.flashs[0]; package_names.len()]
        } else {
            parsed.flashs.clone()
        }
    };

    let group_idx = package_names.iter().find_map(|package_name| {
        let chip_name = chip_name_from_package_name(package_name);
        chips.get(&chip_name).map(|chip| chip.group_idx)
    });

    let group_idx = group_idx.unwrap_or_else(|| {
        let group_idx = chip_groups.len();
        chip_groups.push(ChipGroup {
            chip_names: Vec::new(),
            xml: parsed.clone(),
            ips: HashMap::new(),
            pins: HashMap::new(),
            family: None,
            line: None,
            die: None,
        });
        group_idx
    });

    for (package_i, package_name) in package_names.iter().enumerate() {
        let chip_name = chip_name_from_package_name(package_name);
        if !chips.contains_key(&chip_name) {
            chips.insert(
                chip_name.clone(),
                Chip {
                    flash: package_flashes[package_i],
                    ram: package_rams[package_i],
                    group_idx,
                    packages: Vec::new(),
                },
            );
        }
        chips
            .get_mut(&chip_name)
            .unwrap()
            .packages
            .push(stm32_data_serde::chip::Package {
                name: package_name.clone(),
                package: parsed.package.clone(),
            });
    }

    // Some packages have some peripehrals removed because the package had to
    // remove GPIOs useful for that peripheral. So we merge all peripherals from all packages.
    let group = &mut chip_groups[group_idx];
    for ip in parsed.ips {
        group.ips.insert(ip.instance_name.clone(), ip);
    }
    for pin in parsed.pins {
        if let Some(pin_name) = gpio_af::clean_pin(&pin.name) {
            group
                .pins
                .entry(pin_name)
                .and_modify(|p| {
                    // merge signals.
                    p.signals.extend_from_slice(&pin.signals);
                    p.signals.dedup();
                })
                .or_insert(pin);
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_group(
    mut group: ChipGroup,
    peri_matcher: &mut PeriMatcher,
    headers: &header::Headers,
    af: &gpio_af::Af,
    chip_interrupts: &interrupts::ChipInterrupts,
    peripheral_to_clock: &rcc::ParsedRccs,
    dma_channels: &dma::DmaChannels,
    chips: &HashMap<String, Chip>,
    memories: &memory::Memories,
    docs: &docs::Docs,
) -> Result<(), anyhow::Error> {
    let chip_name = group.chip_names[0].clone();
    group.family = Some(group.xml.family.clone());
    group.line = Some(group.xml.line.clone());
    group.die = Some(group.xml.die.clone());
    let rcc_kind = group.ips.values().find(|x| x.name == "RCC").unwrap().version.clone();
    let rcc_block = peri_matcher
        .match_peri(&format!("{chip_name}:RCC:{rcc_kind}"))
        .unwrap_or_else(|| panic!("could not get rcc for {}", &chip_name));
    let h = headers
        .get_for_chip(&chip_name)
        .unwrap_or_else(|| panic!("could not get header for {}", &chip_name));
    let chip_af = &group.ips.values().find(|x| x.name == "GPIO").unwrap().version;
    let chip_af = chip_af.strip_suffix("_gpio_v1_0").unwrap();
    let chip_af = af.0.get(chip_af);
    let cores: Vec<_> = group
        .xml
        .cores
        .iter()
        .map(|core_xml| {
            process_core(
                core_xml,
                h,
                &chip_name,
                &group,
                chip_interrupts,
                peri_matcher,
                peripheral_to_clock,
                rcc_block,
                chip_af,
                dma_channels,
            )
        })
        .collect();

    for chip_name in &group.chip_names {
        process_chip(chips, chip_name, h, memories, docs, &group, &cores)?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_core(
    core_xml: &str,
    h: &header::ParsedHeader,
    chip_name: &str,
    group: &ChipGroup,
    chip_interrupts: &interrupts::ChipInterrupts,
    peri_matcher: &mut PeriMatcher,
    peripheral_to_clock: &rcc::ParsedRccs,
    rcc_block: (&str, &str, &str),
    chip_af: Option<&HashMap<String, Vec<stm32_data_serde::chip::core::peripheral::Pin>>>,
    dma_channels: &dma::DmaChannels,
) -> stm32_data_serde::chip::Core {
    let core_name = corename(core_xml);
    let defines = h.get_defines(&core_name);

    let mut peri_kinds = HashMap::new();
    peri_kinds.insert("UID".to_string(), "UID".to_string());
    for ip in group.ips.values() {
        let pname = ip.instance_name.clone();
        let pkind = format!("{}:{}", ip.name, ip.version);
        let pkind = pkind.strip_suffix("_Cube").unwrap_or(&pkind);

        const FAKE_PERIPHERALS: &[&str] = &[
            // These are real peripherals but with special handling
            "NVIC",
            "GPIO",
            "DMA",
            // IRTIM is just TIM16+TIM17
            "IRTIM",
            // We add this as ghost peri
            "SYS",
            // These are software libraries
            "FREERTOS",
            "PDM2PCM",
            "FATFS",
            "LIBJPEG",
            "MBEDTLS",
            "LWIP",
            "USB_HOST",
            "USB_DEVICE",
            "GUI_INTERFACE",
            "TRACER_EMB",
            "TOUCHSENSING",
        ];

        if FAKE_PERIPHERALS.contains(&pname.as_str()) {
            continue;
        }

        let pname = match pname.as_str() {
            "HDMI_CEC" => "CEC".to_string(),
            "SUBGHZ" => "SUBGHZSPI".to_string(),
            // remove when https://github.com/stm32-rs/stm32-rs/pull/789 merges
            "USB_DRD_FS" => "USB".to_string(),
            _ => pname,
        };

        if pname.starts_with("ADC") {
            if let Entry::Vacant(entry) = peri_kinds.entry("ADC_COMMON".to_string()) {
                entry.insert(format!("ADC_COMMON:{}", ip.version.strip_suffix("_Cube").unwrap()));
            }
        }
        if pname.starts_with("ADC3") && (chip_name.starts_with("STM32H7") || chip_name.starts_with("STM32F3")) {
            if let Entry::Vacant(entry) = peri_kinds.entry("ADC3_COMMON".to_string()) {
                entry.insert(format!("ADC3_COMMON:{}", ip.version.strip_suffix("_Cube").unwrap()));
            }
        }
        peri_kinds.insert(pname, pkind.to_string());
    }
    const GHOST_PERIS: &[&str] = &[
        "GPIOA",
        "GPIOB",
        "GPIOC",
        "GPIOD",
        "GPIOE",
        "GPIOF",
        "GPIOG",
        "GPIOH",
        "GPIOI",
        "GPIOJ",
        "GPIOK",
        "GPIOL",
        "GPIOM",
        "GPION",
        "GPIOO",
        "GPIOP",
        "GPIOQ",
        "GPIOR",
        "GPIOS",
        "GPIOT",
        "DMA1",
        "DMA2",
        "BDMA",
        "DMAMUX",
        "DMAMUX1",
        "DMAMUX2",
        "SBS",
        "SYSCFG",
        "EXTI",
        "FLASH",
        "DBGMCU",
        "CRS",
        "PWR",
        "AFIO",
        "BKP",
        "USBRAM",
        "VREFINTCAL",
    ];
    for pname in GHOST_PERIS {
        if let Entry::Vacant(entry) = peri_kinds.entry(pname.to_string()) {
            if defines.get_peri_addr(pname).is_some() {
                entry.insert("unknown".to_string());
            }
        }
    }
    if peri_kinds.contains_key("BDMA1") {
        peri_kinds.remove("BDMA");
    }
    let fdcans = peri_kinds
        .keys()
        .filter_map(|pname| {
            regex!(r"^FDCAN(?P<idx>[0-9]+)$")
                .captures(pname)
                .map(|cap| cap["idx"].to_string())
        })
        .collect::<Vec<_>>();
    if !fdcans.is_empty() {
        if chip_name.starts_with("STM32H7") {
            // H7 has one message RAM shared between FDCANs
            peri_kinds
                .entry("FDCANRAM".to_string())
                .or_insert("unknown".to_string());
        } else {
            // Other chips with FDCANs have separate message RAM per module
            for fdcan in fdcans {
                peri_kinds
                    .entry(format!("FDCANRAM{}", fdcan))
                    .or_insert("unknown".to_string());
            }
        }
    }
    // get possible used GPIOs for each peripheral from the chip xml
    // it's not the full info we would want (stuff like AFIO info which comes from GPIO xml),
    //   but we actually need to use it because of F1 line
    //       which doesn't include non-remappable peripherals in GPIO xml
    //   and some weird edge cases like STM32F030C6 (see merge_periph_pins_info)
    let mut periph_pins = HashMap::<_, Vec<_>>::new();
    for (pin_name, pin) in &group.pins {
        for signal in &pin.signals {
            let mut signal = signal.name.clone();
            if signal.starts_with("DEBUG_SUBGHZSPI-") {
                signal = format!("SUBGHZSPI_{}", &signal[16..(signal.len() - 3)]);
            }
            // TODO: What are those signals (well, GPIO is clear) Which peripheral do they belong to?
            if !["GPIO", "CEC", "AUDIOCLK", "VDDTCXO"].contains(&signal.as_str()) && !signal.contains("EXTI") {
                // both peripherals and signals can have underscores in their names so there is no easy way to split
                // check if signal name starts with one of the peripheral names
                for periph in peri_kinds.keys() {
                    if let Some(signal) = signal.strip_prefix(&format!("{periph}_")) {
                        periph_pins.entry(periph.to_string()).or_default().push(
                            stm32_data_serde::chip::core::peripheral::Pin {
                                pin: pin_name.clone(),
                                signal: signal.to_string(),
                                af: None,
                            },
                        );
                        break;
                    }
                }
            }
        }
    }
    for pins in periph_pins.values_mut() {
        pins.sort();
        pins.dedup();
    }
    let mut peripherals = HashMap::new();
    for (pname, pkind) in peri_kinds {
        // We cannot add this to FAKE peripherals because we need the pins
        if pname.starts_with("I2S") {
            continue;
        }

        let addr = if (chip_name.starts_with("STM32F0")
            || chip_name.starts_with("STM32L1")
            || chip_name.starts_with("STM32L0"))
            && pname == "ADC"
        {
            defines.get_peri_addr("ADC1")
        } else if chip_name.starts_with("STM32H7") && pname == "HRTIM" {
            defines.get_peri_addr("HRTIM1")
        } else if let Some(cap) = regex!(r"^FDCANRAM(?P<idx>[0-9]+)$").captures(&pname) {
            defines.get_peri_addr("FDCANRAM").map(|addr| {
                if chip_name.starts_with("STM32H7") {
                    addr
                } else {
                    let idx = cap["idx"].parse::<u32>().unwrap();
                    // FIXME: this offset should not be hardcoded, but I think
                    // it appears in no data sources (only in RMs)
                    addr + (idx - 1) * 0x350
                }
            })
        } else {
            defines.get_peri_addr(&pname)
        };

        let addr = match addr {
            Some(addr) => addr,
            None => continue,
        };

        let mut p = stm32_data_serde::chip::core::Peripheral {
            name: if pname == "SBS" {
                "SYSCFG".to_string()
            } else {
                pname.clone()
            },
            address: addr,
            registers: None,
            rcc: None,
            interrupts: None,
            dma_channels: Vec::new(),
            pins: Vec::new(),
        };

        if let Some(block) = peri_matcher.match_peri(&format!("{chip_name}:{pname}:{pkind}")) {
            p.registers = Some(stm32_data_serde::chip::core::peripheral::Registers {
                kind: block.0.to_string(),
                version: block.1.to_string(),
                block: block.2.to_string(),
            });
        }

        if let Some(rcc_info) = peripheral_to_clock.match_peri_clock(rcc_block.1, &pname) {
            p.rcc = Some(rcc_info);
        }
        if let Some(pins) = periph_pins.get_mut(&pname) {
            // merge the core xml info with GPIO xml info to hopefully get the full picture
            // if the peripheral does not exist in the GPIO xml (one of the notable one is ADC)
            //   it probably doesn't need any AFIO writes to work
            if let Some(af_pins) = chip_af.and_then(|x| x.get(&pname)) {
                merge_periph_pins_info(chip_name, &pname, pins, af_pins.as_slice());
            }
            p.pins = pins.clone();
        }

        let i2s_name = if pname.starts_with("SPI") {
            "I2S".to_owned() + pname.trim_start_matches("SPI")
        } else {
            "".to_owned()
        };

        if let Some(i2s_pins) = periph_pins.get_mut(&i2s_name) {
            // merge the core xml info with GPIO xml info to hopefully get the full picture
            // if the peripheral does not exist in the GPIO xml (one of the notable one is ADC)
            //   it probably doesn't need any AFIO writes to work
            if let Some(af_pins) = chip_af.and_then(|x| x.get(&i2s_name)) {
                merge_periph_pins_info(chip_name, &i2s_name, i2s_pins, af_pins.as_slice());
            }

            p.pins.extend(i2s_pins.iter().map(|p| Pin {
                pin: p.pin.clone(),
                signal: "I2S_".to_owned() + &p.signal,
                af: p.af,
            }));
        }

        // H7 has some _C pin variants (e.g. PC2 and PC2_C). Digital stuff should always be in the non-C pin.
        // cubedb puts it either in both, or in the -C pin only! (in chips where the package has only the -C pin)
        // so we fix that up here.
        if !pname.starts_with("ADC") && !pname.starts_with("DAC") && !pname.starts_with("COMP") {
            for pin in &mut p.pins {
                if let Some(p) = pin.pin.strip_suffix("_C") {
                    pin.pin = p.to_string();
                }
            }
        }

        // sort pins to avoid diff for c pins
        // put the ones with AF number first, so we keep them.
        p.pins
            .sort_by_key(|x| (x.pin.clone(), x.signal.clone(), x.af.is_none()));
        p.pins.dedup_by_key(|x| (x.pin.clone(), x.signal.clone()));

        peripherals.insert(p.name.clone(), p);
    }
    if let Ok(extra_f) = std::fs::read(format!("data/extra/family/{}.yaml", group.family.as_ref().unwrap())) {
        #[derive(serde::Deserialize)]
        struct Extra {
            peripherals: Vec<stm32_data_serde::chip::core::Peripheral>,
        }

        let extra: Extra = serde_yaml::from_slice(&extra_f).unwrap();
        for mut p in extra.peripherals {
            if let Some(peripheral) = peripherals.get_mut(&p.name) {
                // Modify the generated peripheral
                peripheral.pins.append(&mut p.pins);
            } else if p.address != 0 {
                // Only insert the peripheral if the address is not the default
                peripherals.insert(p.name.clone(), p);
            }
        }
    }

    let have_peris: HashSet<_> = peripherals.keys().cloned().collect();
    let mut peripherals: Vec<_> = peripherals.into_values().collect();
    peripherals.sort_by_key(|x| x.name.clone());
    // Collect DMA versions in the chip
    let mut chip_dmas: Vec<_> = group
        .ips
        .values()
        .filter_map(|ip| {
            let version = &ip.version;
            let sort = match ip.name.as_str() {
                "DMA" => 1,
                "BDMA" => 2,
                "BDMA1" => 3,
                "BDMA2" => 4,
                "GPDMA" => 5,
                _ => 0,
            };
            if sort > 0 && dma_channels.0.contains_key(version) {
                Some((sort, version.clone()))
            } else {
                None
            }
        })
        .collect();
    chip_dmas.sort();
    chip_dmas.dedup();
    let chip_dmas: Vec<_> = chip_dmas.into_iter().map(|(_sort, version)| version).collect();
    // Process DMA channels
    let chs = chip_dmas
        .iter()
        .flat_map(|dma| dma_channels.0.get(dma).unwrap().channels.clone());
    // The dma_channels[xx] is generic for multiple chips. The current chip may have less DMAs,
    // so we have to filter it.
    let chs: Vec<_> = chs.filter(|ch| have_peris.contains(&ch.dma)).collect();
    let core_dma_channels = chs.clone();
    let have_chs: HashSet<_> = chs.into_iter().collect();
    // Process peripheral - DMA channel associations
    for p in &mut peripherals {
        let mut chs = Vec::new();
        for dma in &chip_dmas {
            let mut peri_chs = dma_channels.0.get(dma).unwrap().peripherals.get(&p.name);

            // DAC1 is sometimes interchanged with DAC
            if peri_chs.is_none() && p.name == "DAC1" {
                peri_chs = dma_channels.0.get(dma).unwrap().peripherals.get("DAC");
            }

            if let Some(peri_chs) = peri_chs {
                chs.extend(
                    peri_chs
                        .iter()
                        .filter(|ch| {
                            if let Some(ch_channel) = &ch.channel {
                                have_chs.iter().any(|x| &x.name == ch_channel)
                            } else {
                                true
                            }
                        })
                        .cloned(),
                );
            }
        }
        if !chs.is_empty() {
            chs.sort_by_key(|ch| (ch.channel.clone(), ch.dmamux.clone(), ch.request));
            p.dma_channels = chs;
        }
    }

    let mut core = stm32_data_serde::chip::Core {
        name: core_name.clone(),
        peripherals,
        nvic_priority_bits: None,
        interrupts: vec![],
        dma_channels: core_dma_channels,
    };

    chip_interrupts.process(&mut core, chip_name, h, group);

    core
}

fn process_chip(
    chips: &HashMap<String, Chip>,
    chip_name: &str,
    h: &header::ParsedHeader,
    memories: &memory::Memories,
    docs: &docs::Docs,
    group: &ChipGroup,
    cores: &[stm32_data_serde::chip::Core],
) -> Result<(), anyhow::Error> {
    let chip = chips.get(chip_name).unwrap();
    let flash_size = chip.flash * 1024;
    let ram_total = chip.ram * 1024;
    let memory = memories.get(group.die.as_ref().unwrap());
    let mut flash_remaining = flash_size;
    let mut memory_regions = Vec::new();
    let mut found = HashSet::<&str>::new();
    for each in [
        // We test FLASH_BANKx _before_ FLASH as we prefer their definition over the legacy one
        "FLASH_BANK1",
        "FLASH_BANK2",
        "FLASH",
        "FLASH_OTP",
        "D1_AXIFLASH",
        "D1_AXIICP",
    ] {
        if let Some(address) = h.defines.get("all").unwrap().0.get(&format!("{each}_BASE")) {
            let (key, banks) = match each {
                "FLASH" => (
                    "BANK_1",
                    Some([memory::FlashBank::Bank1, memory::FlashBank::Bank2].as_ref()),
                ),
                "FLASH_BANK1" => ("BANK_1", Some([memory::FlashBank::Bank1].as_ref())),
                "FLASH_BANK2" => ("BANK_2", Some([memory::FlashBank::Bank2].as_ref())),
                "FLASH_OTP" => ("OTP", Some([memory::FlashBank::Otp].as_ref())),
                each => (each, None),
            };

            if found.contains(key) {
                continue;
            }
            found.insert(key);

            if let Some(banks) = banks {
                for bank in banks {
                    let bank_name = match bank {
                        memory::FlashBank::Bank1 => "BANK_1",
                        memory::FlashBank::Bank2 => "BANK_2",
                        memory::FlashBank::Otp => "OTP",
                    };
                    let regions: Vec<_> = memory
                        .flash_regions
                        .iter()
                        .filter(|region| region.bank == *bank)
                        .enumerate()
                        .map_while(|(index, region)| {
                            let size = if *bank == memory::FlashBank::Bank1 || *bank == memory::FlashBank::Bank2 {
                                // Truncate region to the total amount of remaining chip flash
                                let size = std::cmp::min(region.bytes, flash_remaining);
                                flash_remaining -= size;
                                if size == 0 {
                                    // No more regions are present on this chip
                                    return None;
                                }
                                size
                            } else {
                                region.bytes
                            };

                            Some((index, region.address, size, region.settings.clone()))
                        })
                        .collect();
                    let has_multiple_regions = regions.len() > 1;
                    for (index, address, size, settings) in regions {
                        let name = if has_multiple_regions {
                            format!("{}_REGION_{}", bank_name, index + 1)
                        } else {
                            bank_name.to_string()
                        };

                        memory_regions.push(stm32_data_serde::chip::Memory {
                            name,
                            kind: stm32_data_serde::chip::memory::Kind::Flash,
                            address,
                            size,
                            settings: Some(settings.clone()),
                        });
                    }
                }
            } else {
                memory_regions.push(stm32_data_serde::chip::Memory {
                    name: key.to_string(),
                    kind: stm32_data_serde::chip::memory::Kind::Flash,
                    address: u32::try_from(*address).unwrap(),
                    size: 0,
                    settings: None,
                })
            }
        }
    }
    let mut found = HashSet::new();
    for each in [
        "SRAM",
        "SRAM1",
        "SRAM2",
        "D1_AXISRAM",
        "D1_ITCMRAM",
        "D1_DTCMRAM",
        "D1_AHBSRAM",
        "D2_AXISRAM",
        "D3_BKPSRAM",
        "D3_SRAM",
    ] {
        if let Some(address) = h.defines.get("all").unwrap().0.get(&format!("{each}_BASE")) {
            let key = match each {
                "D1_AXISRAM" => "SRAM",
                "SRAM1" => "SRAM",
                each => each,
            };

            if found.contains(key) {
                continue;
            }
            found.insert(key);

            let size = if key == "SRAM" {
                // if memory.ram.bytes != ram_total {
                //     println!(
                //         "SRAM mismatch for chip {} with die {}: Expected {} was {}",
                //         chip_name,
                //         group.die.as_ref().unwrap(),
                //         ram_total,
                //         memory.ram.bytes,
                //     );
                // }
                std::cmp::min(memory.ram.bytes, ram_total)
            } else {
                0
            };

            memory_regions.push(stm32_data_serde::chip::Memory {
                name: key.to_string(),
                kind: stm32_data_serde::chip::memory::Kind::Ram,
                address: u32::try_from(*address).unwrap(),
                size,
                settings: None,
            })
        }
    }
    let docs = docs.documents_for(chip_name);
    let chip = stm32_data_serde::Chip {
        name: chip_name.to_string(),
        family: group.family.clone().unwrap(),
        line: group.line.clone().unwrap(),
        die: group.die.clone().unwrap(),
        device_id: memory.device_id,
        packages: chip.packages.clone(),
        memory: memory_regions,
        docs,
        cores: cores.to_vec(),
    };
    let dump = serde_json::to_string_pretty(&chip)?;

    // TODO: delete this.
    // This makes the formating match the output of the original python script, to prevent unnecessary churn
    let dump = {
        let mut cleaned = String::new();
        for line in dump.lines() {
            let spaces = line.bytes().take_while(|b| *b == b' ').count();
            for _ in 0..spaces {
                // add an extra space for every existing space
                // this converts two-space indents to four-space indents
                cleaned.push(' ');
            }
            // escape non-ascii symbols
            let line = line.replace('\u{00ae}', r"\u00ae");
            let line = line.replace('\u{2122}', r"\u2122");
            cleaned.push_str(&line);
            cleaned.push('\n');
        }
        // remove trailing newline
        cleaned.pop();
        cleaned
    };

    std::fs::write(format!("build/data/chips/{chip_name}.json"), dump)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn dump_all_chips(
    chip_groups: Vec<ChipGroup>,
    headers: header::Headers,
    af: gpio_af::Af,
    chip_interrupts: interrupts::ChipInterrupts,
    peripheral_to_clock: rcc::ParsedRccs,
    dma_channels: dma::DmaChannels,
    chips: std::collections::HashMap<String, Chip>,
    memories: memory::Memories,
    docs: docs::Docs,
) -> Result<(), anyhow::Error> {
    std::fs::create_dir_all("build/data/chips")?;

    #[cfg(feature = "rayon")]
    {
        use rayon::prelude::*;

        chip_groups
            .into_par_iter()
            .try_for_each_init(PeriMatcher::new, |peri_matcher, group| {
                process_group(
                    group,
                    peri_matcher,
                    &headers,
                    &af,
                    &chip_interrupts,
                    &peripheral_to_clock,
                    &dma_channels,
                    &chips,
                    &memories,
                    &docs,
                )
            })
    }
    #[cfg(not(feature = "rayon"))]
    {
        let mut peri_matcher = PeriMatcher::new();

        chip_groups.into_iter().try_for_each(|group| {
            process_group(
                group,
                &mut peri_matcher,
                &headers,
                &af,
                &chip_interrupts,
                &peripheral_to_clock,
                &dma_channels,
                &chips,
                &memories,
                &docs,
            )
        })
    }
}
