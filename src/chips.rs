use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

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
    ips: HashMap<String, xml::Ip>,
    pins: HashMap<stm32_data_serde::chip::core::peripheral::pin::Pin, xml::Pin>,
    family: Option<String>,
    line: Option<String>,
    die: Option<String>,
}

fn chip_name_from_package_name(x: &str) -> String {
    let regexes = [
        (regex!("^(STM32L1....).x([AX])$"), "$1-$2"),
        (regex!("^(STM32G0....).xN$"), "$1"),
        (regex!("^(STM32F412..).xP$"), "$1"),
        (regex!("^(STM32L4....).xP$"), "$1"),
        (regex!("^(STM32WB....).x[AE]$"), "$1"),
        (regex!("^(STM32G0....).xN$"), "$1"),
        (regex!("^(STM32L5....).x[PQ]$"), "$1"),
        (regex!("^(STM32L0....).xS$"), "$1"),
        (regex!("^(STM32H7....).xQ$"), "$1"),
        (regex!("^(STM32U5....).xQ$"), "$1"),
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
            (".*:USART:sci2_v1_2", ("usart", "v1", "USART")),
            (".*:USART:sci2_v2_0", ("usart", "v2", "USART")),
            (".*:USART:sci2_v2_1", ("usart", "v2", "USART")),
            (".*:USART:sci2_v2_2", ("usart", "v2", "USART")),
            (".*:USART:sci3_v1_0", ("usart", "v2", "USART")),
            (".*:USART:sci3_v1_1", ("usart", "v2", "USART")),
            (".*:USART:sci3_v1_2", ("usart", "v2", "USART")),
            (".*:USART:sci3_v2_0", ("usart", "v2", "USART")),
            (".*:USART:sci3_v2_1", ("usart", "v2", "USART")),
            (".*:UART:sci2_v1_2_F4", ("usart", "v1", "USART")),
            (".*:UART:sci2_v2_1", ("usart", "v2", "USART")),
            (".*:UART:sci2_v3_0", ("usart", "v2", "USART")),
            (".*:UART:sci2_v3_1", ("usart", "v2", "USART")),
            (".*:LPUART:sci3_v1_1", ("lpuart", "v1", "LPUART")),
            (".*:LPUART:sci3_v1_2", ("lpuart", "v2", "LPUART")),
            (".*:LPUART:sci3_v1_3", ("lpuart", "v2", "LPUART")),
            (".*:LPUART:sci3_v1_4", ("lpuart", "v2", "LPUART")),
            (".*:RNG:rng1_v1_1", ("rng", "v1", "RNG")),
            (".*:RNG:rng1_v2_0", ("rng", "v1", "RNG")),
            (".*:RNG:rng1_v2_1", ("rng", "v1", "RNG")),
            (".*:RNG:rng1_v3_1", ("rng", "v1", "RNG")),
            (".*:SPI:spi2_v1_4", ("spi", "f1", "SPI")),
            (".*:SPI:spi2s1_v2_1", ("spi", "v1", "SPI")),
            (".*:SPI:spi2s1_v2_2", ("spi", "v1", "SPI")),
            (".*:SPI:spi2s1_v3_2", ("spi", "v2", "SPI")),
            (".*:SPI:spi2s1_v3_3", ("spi", "v2", "SPI")),
            (".*:SPI:spi2s1_v3_5", ("spi", "v2", "SPI")),
            (".*:SUBGHZSPI:.*", ("spi", "v2", "SPI")),
            (".*:SPI:spi2s1_v3_1", ("spi", "v2", "SPI")),
            (".*:SPI:spi2s2_v1_1", ("spi", "v3", "SPI")),
            (".*:SPI:spi2s2_v1_0", ("spi", "v3", "SPI")),
            (".*:SPI:spi2s3_v1_1", ("spi", "v4", "SPI")),
            (".*:I2C:i2c1_v1_5", ("i2c", "v1", "I2C")),
            (".*:I2C:i2c2_v1_1", ("i2c", "v2", "I2C")),
            (".*:I2C:i2c2_v1_1F7", ("i2c", "v2", "I2C")),
            (".*:I2C:i2c2_v1_1U5", ("i2c", "v2", "I2C")),
            (".*:DAC:dacif_v1_1", ("dac", "v1", "DAC")),
            (".*:DAC:dacif_v1_1F1", ("dac", "v1", "DAC")),
            (".*:DAC:F0dacif_v1_1", ("dac", "v1", "DAC")),
            (".*:DAC:dacif_v2_0", ("dac", "v2", "DAC")),
            (".*:DAC:dacif_v3_0", ("dac", "v2", "DAC")),
            (".*:ADC:aditf_v2_5F1", ("adc", "f1", "ADC")),
            (".*:ADC:aditf4_v1_1", ("adc", "v1", "ADC")),
            (".*:ADC:aditf2_v1_1", ("adc", "v2", "ADC")),
            (".*:ADC:aditf5_v2_0", ("adc", "v3", "ADC")),
            (".*:ADC:aditf5_v3_0", ("adc", "v4", "ADC")),
            ("STM32G0.*:ADC:.*", ("adc", "g0", "ADC")),
            ("STM32G0.*:ADC_COMMON:.*", ("adccommon", "v3", "ADC_COMMON")),
            (".*:ADC_COMMON:aditf2_v1_1", ("adccommon", "v2", "ADC_COMMON")),
            (".*:ADC_COMMON:aditf5_v2_0", ("adccommon", "v3", "ADC_COMMON")),
            (".*:ADC_COMMON:aditf4_v3_0_WL", ("adccommon", "v3", "ADC_COMMON")),
            ("STM32H7.*:ADC_COMMON:.*", ("adccommon", "v4", "ADC_COMMON")),
            ("STM32H7.*:ADC3_COMMON:.*", ("adccommon", "v4", "ADC_COMMON")),
            (".*:DCMI:.*", ("dcmi", "v1", "DCMI")),
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
            ("STM32H7.*:SYSCFG:.*", ("syscfg", "h7", "SYSCFG")),
            ("STM32U5.*:SYSCFG:.*", ("syscfg", "u5", "SYSCFG")),
            ("STM32WB.*:SYSCFG:.*", ("syscfg", "wb", "SYSCFG")),
            ("STM32WL5.*:SYSCFG:.*", ("syscfg", "wl5", "SYSCFG")),
            ("STM32WLE.*:SYSCFG:.*", ("syscfg", "wle", "SYSCFG")),
            (".*:IWDG:iwdg1_v1_1", ("iwdg", "v1", "IWDG")),
            (".*:IWDG:iwdg1_v2_0", ("iwdg", "v2", "IWDG")),
            (".*:WWDG:wwdg1_v1_0", ("wwdg", "v1", "WWDG")),
            (".*:JPEG:jpeg1_v1_0", ("jpeg", "v1", "JPEG")),
            (".*:LPTIM:F7_lptimer1_v1_1", ("lptim", "v1", "LPTIM")),
            (".*:LTDC:lcdtft1_v1_1", ("ltdc", "v1", "LTDC")),
            (".*:MDIOS:mdios1_v1_0", ("mdios", "v1", "MDIOS")),
            (".*:QUADSPI:quadspi1_v1_0", ("quadspi", "v1", "QUADSPI")),
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
            ("STM32WB.*:RTC:rtc2_.*", ("rtc", "v2wb", "RTC")),
            ("STM32U5.*:RTC:rtc2_.*", ("rtc", "v3u5", "RTC")), // Cube says v2, but it's v3 with security stuff
            (".*:RTC:rtc3_v1_0", ("rtc", "v3", "RTC")),
            (".*:RTC:rtc3_v1_1", ("rtc", "v3", "RTC")),
            (".*:RTC:rtc3_v2_0", ("rtc", "v3", "RTC")),
            (".*:RTC:rtc3_v3_0", ("rtc", "v3", "RTC")),
            (".*:SAI:sai1_v1_1", ("sai", "v1", "SAI")),
            (".*:SDIO:sdmmc_v1_2", ("sdmmc", "v1", "SDMMC")),
            (".*:SDMMC:sdmmc_v1_3", ("sdmmc", "v1", "SDMMC")),
            (".*:SPDIFRX:spdifrx1_v1_0", ("spdifrx", "v1", "SPDIFRX")),
            // # USB
            ("STM32F302.[BC].*:USB:.*", ("usb", "v1_x1", "USB")),
            ("STM32F302.[68DE].*:USB:.*", ("usb", "v2", "USB")),
            ("STM32F303.[BC].*:USB:.*", ("usb", "v1_x1", "USB")),
            ("STM32F303.[DE].*:USB:.*", ("usb", "v2", "USB")),
            ("STM32F373.*:USB:.*", ("usb", "v1_x2", "USB")),
            ("STM32(F1|L1).*:USB:.*", ("usb", "v1_x1", "USB")),
            ("STM32G0[BC]1.*:USB:.*", ("usb", "v4", "USB")),
            (".*:USB:.*", ("usb", "v3", "USB")),
            // # USB OTG
            (".*:USB_OTG_FS:otgfs1_.*", ("otg", "v1", "OTG")),
            (".*:USB_OTG_HS:otghs1_.*", ("otg", "v1", "OTG")),
            ("STM32F0.*:RCC:.*", ("rcc", "f0", "RCC")),
            ("STM32F100.*:RCC:.*", ("rcc", "f100", "RCC")),
            ("STM32F10[123].*:RCC:.*", ("rcc", "f1", "RCC")),
            ("STM32F10[57].*:RCC:.*", ("rcc", "f1cl", "RCC")),
            ("STM32F2.*:RCC:.*", ("rcc", "f2", "RCC")),
            ("STM32F3.*:RCC:.*", ("rcc", "f3", "RCC")),
            ("STM32F410.*:RCC:.*", ("rcc", "f410", "RCC")),
            ("STM32F4.*:RCC:.*", ("rcc", "f4", "RCC")),
            ("STM32F7.*:RCC:.*", ("rcc", "f7", "RCC")),
            ("STM32G0.*:RCC:.*", ("rcc", "g0", "RCC")),
            ("STM32G4.*:RCC:.*", ("rcc", "g4", "RCC")),
            ("STM32H7[AB].*:RCC:.*", ("rcc", "h7ab", "RCC")),
            ("STM32H7.*:RCC:.*", ("rcc", "h7", "RCC")),
            ("STM32L0.*:RCC:.*", ("rcc", "l0", "RCC")),
            ("STM32L1.*:RCC:.*", ("rcc", "l1", "RCC")),
            ("STM32L4.*:RCC:.*", ("rcc", "l4", "RCC")),
            ("STM32L5.*:RCC:.*", ("rcc", "l5", "RCC")),
            ("STM32U5.*:RCC:.*", ("rcc", "u5", "RCC")),
            ("STM32WB.*:RCC:.*", ("rcc", "wb", "RCC")),
            ("STM32WL5.*:RCC:.*", ("rcc", "wl5", "RCC")),
            ("STM32WLE.*:RCC:.*", ("rcc", "wle", "RCC")),
            ("STM32F1.*:SPI[1234]:.*", ("spi", "f1", "SPI")),
            ("STM32F3.*:SPI[1234]:.*", ("spi", "v2", "SPI")),
            ("STM32F1.*:AFIO:.*", ("afio", "f1", "AFIO")),
            ("STM32L5.*:EXTI:.*", ("exti", "l5", "EXTI")),
            ("STM32G0.*:EXTI:.*", ("exti", "g0", "EXTI")),
            ("STM32H7.*:EXTI:.*", ("exti", "h7", "EXTI")),
            ("STM32U5.*:EXTI:.*", ("exti", "u5", "EXTI")),
            ("STM32WB.*:EXTI:.*", ("exti", "w", "EXTI")),
            ("STM32WL5.*:EXTI:.*", ("exti", "w", "EXTI")),
            ("STM32WLE.*:EXTI:.*", ("exti", "wle", "EXTI")),
            (".*:EXTI:.*", ("exti", "v1", "EXTI")),
            ("STM32L0.*:CRS:.*", ("crs", "l0", "CRS")),
            (".*SDMMC:sdmmc2_v1_0", ("sdmmc", "v2", "SDMMC")),
            ("STM32G0.*:PWR:.*", ("pwr", "g0", "PWR")),
            ("STM32G4.*:PWR:.*", ("pwr", "g4", "PWR")),
            ("STM32H7(42|43|53|50).*:PWR:.*", ("pwr", "h7", "PWR")),
            ("STM32H7.*:PWR:.*", ("pwr", "h7smps", "PWR")),
            ("STM32F2.*:PWR:.*", ("pwr", "f2", "PWR")),
            ("STM32F3.*:PWR:.*", ("pwr", "f3", "PWR")),
            ("STM32F4.*:PWR:.*", ("pwr", "f4", "PWR")),
            ("STM32F7.*:PWR:.*", ("pwr", "f7", "PWR")),
            ("STM32L1.*:PWR:.*", ("pwr", "l1", "PWR")),
            ("STM32L4.*:PWR:.*", ("pwr", "l4", "PWR")),
            ("STM32L5.*:PWR:.*", ("pwr", "l5", "PWR")),
            ("STM32U5.*:PWR:.*", ("pwr", "u5", "PWR")),
            ("STM32WL.*:PWR:.*", ("pwr", "wl5", "PWR")),
            ("STM32WB.*:PWR:.*", ("pwr", "wb55", "PWR")),
            ("STM32H7.*:FLASH:.*", ("flash", "h7", "FLASH")),
            ("STM32F0.*:FLASH:.*", ("flash", "f0", "FLASH")),
            ("STM32F1.*:FLASH:.*", ("flash", "f1", "FLASH")),
            ("STM32F2.*:FLASH:.*", ("flash", "f2", "FLASH")),
            ("STM32F3.*:FLASH:.*", ("flash", "f3", "FLASH")),
            ("STM32F4.*:FLASH:.*", ("flash", "f4", "FLASH")),
            ("STM32F7.*:FLASH:.*", ("flash", "f7", "FLASH")),
            ("STM32L0[0-9]2.*:FLASH:.*", ("flash", "l0", "FLASH")),
            ("STM32L1.*:FLASH:.*", ("flash", "l1", "FLASH")),
            ("STM32L4.*:FLASH:.*", ("flash", "l4", "FLASH")),
            ("STM32L5.*:FLASH:.*", ("flash", "l5", "FLASH")),
            ("STM32U5.*:FLASH:.*", ("flash", "u5", "FLASH")),
            ("STM32WB.*:FLASH:.*", ("flash", "wb", "FLASH")),
            ("STM32WL.*:FLASH:.*", ("flash", "wl", "FLASH")),
            ("STM32G0.*:FLASH:.*", ("flash", "g0", "FLASH")),
            ("STM32F107.*:ETH:.*", ("eth", "v1a", "ETH")),
            ("STM32F[24].*:ETH:.*", ("eth", "v1b", "ETH")),
            ("STM32F7.*:ETH:ETH:ethermac110_v2_0", ("eth", "v1c", "ETH")),
            (".*ETH:ethermac110_v3_0", ("eth", "v2", "ETH")),
            (".*:FSMC:.*", ("fsmc", "v1", "FSMC")),
            ("STM32H7.*:FMC:.*", ("fmc", "h7", "FMC")),
            (r".*LPTIM\d.*:G0xx_lptimer1_v1_4", ("lptim", "g0", "LPTIM")),
            ("STM32F1.*:TIM(1|8):.*", ("timer", "v1", "TIM_ADV")),
            ("STM32F1.*:TIM(2|5):.*", ("timer", "v1", "TIM_GP16")),
            ("STM32F1.*:TIM(6|7):.*", ("timer", "v1", "TIM_BASIC")),
            ("STM32L0.*:TIM2:.*", ("timer", "v1", "TIM_GP16")),
            ("STM32U5.*:TIM(2|3|4|5):.*", ("timer", "v1", "TIM_GP32")),
            ("STM32.*:TIM(1|8|20):.*", ("timer", "v1", "TIM_ADV")),
            ("STM32.*:TIM(2|5|23|24):.*", ("timer", "v1", "TIM_GP32")),
            ("STM32.*:TIM(6|7|18):.*", ("timer", "v1", "TIM_BASIC")),
            (r".*TIM\d.*:gptimer.*", ("timer", "v1", "TIM_GP16")),
            ("STM32F0.*:DBGMCU:.*", ("dbgmcu", "f0", "DBGMCU")),
            ("STM32F1.*:DBGMCU:.*", ("dbgmcu", "f1", "DBGMCU")),
            ("STM32F2.*:DBGMCU:.*", ("dbgmcu", "f2", "DBGMCU")),
            ("STM32F3.*:DBGMCU:.*", ("dbgmcu", "f3", "DBGMCU")),
            ("STM32F4.*:DBGMCU:.*", ("dbgmcu", "f4", "DBGMCU")),
            ("STM32F7.*:DBGMCU:.*", ("dbgmcu", "f7", "DBGMCU")),
            ("STM32G0.*:DBGMCU:.*", ("dbgmcu", "g0", "DBGMCU")),
            ("STM32G4.*:DBGMCU:.*", ("dbgmcu", "g4", "DBGMCU")),
            ("STM32H7.*:DBGMCU:.*", ("dbgmcu", "h7", "DBGMCU")),
            ("STM32L0.*:DBGMCU:.*", ("dbgmcu", "l0", "DBGMCU")),
            ("STM32L1.*:DBGMCU:.*", ("dbgmcu", "l1", "DBGMCU")),
            ("STM32L4.*:DBGMCU:.*", ("dbgmcu", "l4", "DBGMCU")),
            ("STM32U5.*:DBGMCU:.*", ("dbgmcu", "u5", "DBGMCU")),
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
            // # stm32F4 CRC peripheral
            // # ("STM32F4*:CRC:CRC:crc_f4")
            // # v1: F1, F2, F4, L1
            // # v2, adds INIT reg: F0
            // # v3, adds POL reg: F3, F7, G0, G4, H7, L0, L4, L5, WB, WL
            (".*:CRC:integtest1_v1_0", ("crc", "v1", "CRC")),
            ("STM32L[04].*:CRC:integtest1_v2_0", ("crc", "v3", "CRC")),
            (".*:CRC:integtest1_v2_0", ("crc", "v2", "CRC")),
            (".*:CRC:integtest1_v2_2", ("crc", "v3", "CRC")),
            (".*:LCD:lcdc1_v1.0.*", ("lcd", "v1", "LCD")),
            (".*:LCD:lcdc1_v1.2.*", ("lcd", "v2", "LCD")),
            (".*:LCD:lcdc1_v1.3.*", ("lcd", "v2", "LCD")),
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
    is_f1: bool,
    periph_name: &str,
    core_pins: &mut Vec<stm32_data_serde::chip::core::peripheral::Pin>,
    af_pins: &[stm32_data_serde::chip::core::peripheral::Pin],
) {
    if is_f1 {
        // TODO: actually handle the F1 AFIO information when it will be extracted
        return;
    }

    // covert to hashmap
    let af_pins: HashMap<(stm32_data_serde::chip::core::peripheral::pin::Pin, &str), Option<u8>> =
        af_pins.iter().map(|v| ((v.pin, v.signal.as_str()), v.af)).collect();

    for pin in core_pins {
        let af = af_pins.get(&(pin.pin, &pin.signal)).copied().flatten();

        // try to look for a signal with another name
        let af = af.or_else(|| {
            if pin.signal == "CTS" {
                // for some godforsaken reason UART4's and UART5's CTS are called CTS_NSS in the GPIO xml
                // so try to match with these
                af_pins.get(&(pin.pin, "CTS_NSS")).copied().flatten()
            } else if periph_name == "I2C1" {
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

fn parse_group(
    f: std::path::PathBuf,
    chips: &mut HashMap<String, Chip>,
    chip_groups: &mut Vec<ChipGroup>,
) -> anyhow::Result<()> {
    let ff = f.file_name().unwrap().to_string_lossy();

    // Not supported
    if ff.contains("STM32MP") {
        return Ok(());
    }

    // Does not exist in ST website. No datasheet, no RM.
    if ff.contains("STM32GBK") || ff.contains("STM32L485") {
        return Ok(());
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
            group.pins.insert(pin_name, pin);
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
    peripheral_to_clock: &rcc::PeripheralToClock,
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
    let rcc_block = peri_matcher.match_peri(&format!("{chip_name}:RCC:{rcc_kind}")).unwrap();
    let h = headers.get_for_chip(&chip_name).unwrap();
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
    peripheral_to_clock: &rcc::PeripheralToClock,
    rcc_block: (&str, &str, &str),
    chip_af: Option<&HashMap<String, Vec<stm32_data_serde::chip::core::peripheral::Pin>>>,
    dma_channels: &dma::DmaChannels,
) -> stm32_data_serde::chip::Core {
    let real_core_name = corename(core_xml);

    let core_name = if !h.interrupts.contains_key(&real_core_name) || !h.defines.contains_key(&real_core_name) {
        "all"
    } else {
        &real_core_name
    };
    // C header defines for this core.
    let defines = h.defines.get(core_name).unwrap();
    // Interrupts!
    let want_nvic_name = {
        // Most chips have a single NVIC, named "NVIC"
        let mut want_nvic_name = "NVIC";

        // Exception 1: Multicore: NVIC1 is the first core, NVIC2 is the second. We have to pick the right one.
        if ["H745", "H747", "H755", "H757", "WL54", "WL55"].contains(&&chip_name[5..9]) {
            if core_name == "cm7" {
                want_nvic_name = "NVIC1";
            } else {
                want_nvic_name = "NVIC2"
            }
        }
        if &chip_name[5..8] == "WL5" {
            if core_name == "cm4" {
                want_nvic_name = "NVIC1";
            } else {
                want_nvic_name = "NVIC2"
            }
        }
        // Exception 2: TrustZone: NVIC1 is Secure mode, NVIC2 is NonSecure mode. For now, we pick the NonSecure one.
        if ["L5", "U5"].contains(&&chip_name[5..7]) {
            want_nvic_name = "NVIC2"
        }

        want_nvic_name
    };
    let chip_nvic = group.ips.values().find(|x| x.name == want_nvic_name).unwrap();
    let mut header_irqs = h.interrupts.get(core_name).unwrap().clone();
    let chip_irqs = chip_interrupts
        .0
        .get(&(chip_nvic.name.clone(), chip_nvic.version.clone()))
        .unwrap();
    // F100xE MISC_REMAP remaps some DMA IRQs, so ST decided to give two names
    // to the same IRQ number.
    if chip_name.starts_with("STM32F100") {
        header_irqs.remove("DMA2_Channel4_5");
    }
    let mut interrupts: Vec<_> = header_irqs
        .iter()
        .map(|(k, v)| stm32_data_serde::chip::core::Interrupt {
            name: k.clone(),
            number: *v,
        })
        .collect();
    interrupts.sort_unstable_by_key(|x| x.number);
    let mut peri_kinds = HashMap::new();
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
            // I2S is just SPI on disguise
            "I2S1",
            "I2S2",
            "I2S3",
            "I2S4",
            "I2S5",
            "I2S6",
            "I2S7",
            "I2S8",
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
        if pname.starts_with("ADC3") && chip_name.starts_with("STM32H7") {
            if let Entry::Vacant(entry) = peri_kinds.entry("ADC3_COMMON".to_string()) {
                entry.insert(format!("ADC3_COMMON:{}", ip.version.strip_suffix("_Cube").unwrap()));
            }
        }
        peri_kinds.insert(pname, pkind.to_string());
    }
    const GHOST_PERIS: &[&str] = &[
        "GPIOA", "GPIOB", "GPIOC", "GPIOD", "GPIOE", "GPIOF", "GPIOG", "GPIOH", "GPIOI", "GPIOJ", "GPIOK", "GPIOL",
        "GPIOM", "GPION", "GPIOO", "GPIOP", "GPIOQ", "GPIOR", "GPIOS", "GPIOT", "DMA1", "DMA2", "BDMA", "DMAMUX",
        "DMAMUX1", "DMAMUX2", "SYSCFG", "EXTI", "FLASH", "DBGMCU", "CRS", "PWR", "AFIO", "BKP",
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
                                pin: *pin_name,
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
    let mut peripherals = Vec::new();
    for (pname, pkind) in peri_kinds {
        let addr = if chip_name.starts_with("STM32F0") && pname == "ADC" {
            defines.get_peri_addr("ADC1")
        } else {
            defines.get_peri_addr(&pname)
        };

        let addr = match addr {
            Some(addr) => addr,
            None => continue,
        };

        let mut p = stm32_data_serde::chip::core::Peripheral {
            name: pname.clone(),
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

        if let Some(rcc_info) = peripheral_to_clock.match_peri_clock(
            (
                rcc_block.0.to_string(),
                rcc_block.1.to_string(),
                rcc_block.2.to_string(),
            ),
            &pname,
        ) {
            p.rcc = Some(rcc_info.clone());
        }
        if let Some(pins) = periph_pins.get_mut(&pname) {
            // merge the core xml info with GPIO xml info to hopefully get the full picture
            // if the peripheral does not exist in the GPIO xml (one of the notable one is ADC)
            //   it probably doesn't need any AFIO writes to work
            if let Some(af_pins) = chip_af.and_then(|x| x.get(&pname)) {
                merge_periph_pins_info(chip_name.contains("STM32F1"), &pname, pins, af_pins.as_slice());
            }
            p.pins = pins.clone();
        }
        if let Some(peri_irqs) = chip_irqs.get(&pname) {
            //filter by available, because some are conditioned on <Die>
            let mut irqs: Vec<_> = peri_irqs
                .iter()
                .filter(|i| header_irqs.contains_key(&i.interrupt))
                .cloned()
                .collect();
            irqs.sort_by_key(|x| (x.signal.clone(), x.interrupt.clone()));
            p.interrupts = Some(irqs);
        }
        peripherals.push(p);
    }
    if let Ok(extra_f) = std::fs::read(format!("data/extra/family/{}.yaml", group.family.as_ref().unwrap())) {
        #[derive(serde::Deserialize)]
        struct Extra {
            peripherals: Vec<stm32_data_serde::chip::core::Peripheral>,
        }

        let extra: Extra = serde_yaml::from_slice(&extra_f).unwrap();
        for p in extra.peripherals {
            peripherals.push(p);
        }
    }
    peripherals.sort_by_key(|x| x.name.clone());
    let have_peris: HashSet<_> = peripherals.iter_mut().map(|p| p.name.clone()).collect();
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
    for mut p in &mut peripherals {
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
    stm32_data_serde::chip::Core {
        name: real_core_name.clone(),
        peripherals,
        interrupts,
        dma_channels: core_dma_channels,
    }
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
    let flash_total = chip.flash * 1024;
    let ram_total = chip.ram * 1024;
    let mut memory_regions = Vec::new();
    let mut found = HashSet::<&str>::new();
    for each in ["FLASH", "FLASH_BANK1", "FLASH_BANK2", "D1_AXIFLASH", "D1_AXIICP"] {
        if let Some(address) = h.defines.get("all").unwrap().0.get(&format!("{each}_BASE")) {
            let key = match each {
                "FLASH" => "BANK_1",
                "FLASH_BANK1" => "BANK_1",
                "FLASH_BANK2" => "BANK_2",
                each => each,
            };

            if found.contains(key) {
                continue;
            }
            found.insert(key);

            let size = if key == "BANK_1" || key == "BANK_2" {
                let size = memories.determine_flash_size(chip_name);
                std::cmp::min(size, flash_total)
            } else {
                0
            };

            memory_regions.push(stm32_data_serde::chip::Memory {
                name: key.to_string(),
                kind: stm32_data_serde::chip::memory::Kind::Flash,
                address: u32::try_from(*address).unwrap(),
                size,
                settings: Some(memories.determine_flash_settings(chip_name)),
            });
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
                let size = memories.determine_ram_size(chip_name);
                std::cmp::min(size, ram_total)
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
    let device_id = memories.determine_device_id(chip_name);
    let chip = stm32_data_serde::Chip {
        name: chip_name.to_string(),
        family: group.family.clone().unwrap(),
        line: group.line.clone().unwrap(),
        die: group.die.clone().unwrap(),
        device_id,
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

    std::fs::write(format!("data/chips/{chip_name}.json"), dump)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn dump_all_chips(
    chip_groups: Vec<ChipGroup>,
    headers: header::Headers,
    af: gpio_af::Af,
    chip_interrupts: interrupts::ChipInterrupts,
    peripheral_to_clock: rcc::PeripheralToClock,
    dma_channels: dma::DmaChannels,
    chips: std::collections::HashMap<String, Chip>,
    memories: memory::Memories,
    docs: docs::Docs,
) -> Result<(), anyhow::Error> {
    std::fs::create_dir_all("data/chips")?;

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
