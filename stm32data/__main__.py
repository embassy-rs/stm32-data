#!/usr/bin/env python3

import sys
import xmltodict
import functools
import re
import json
import os
from glob import glob

from stm32data import yaml, header, interrupts, memory
from stm32data.util import *


def corename(d):
    # print("CHECKING CORENAME", d)
    if m := re.match('.*Cortex-M(\d+)(\+?)\s*(.*)', d):
        name = "cm" + str(m.group(1))
        if m.group(2) == "+":
            name += "p"
        if m.group(3) == "secure":
            name += "s"
        return name


def children(x, key):
    r = x.get(key)
    if r is None:
        return []
    if type(r) is list:
        return r
    return [r]


def expand_name(name):
    if '(' not in name:
        return [name]
    prefix, suffix = name.split('(')
    letters, suffix = suffix.split(')')
    return [prefix + x + suffix for x in letters.split('-')]


# ========================================
# ========================================

FAKE_PERIPHERALS = [
    # These are real peripherals but with special handling
    'NVIC',
    'GPIO',
    'DMA',

    # IRTIM is just TIM16+TIM17
    'IRTIM',

    # I2S is just SPI on disguise
    'I2S1',
    'I2S2',
    'I2S3',
    'I2S4',
    'I2S5',
    'I2S6',
    'I2S7',
    'I2S8',

    # We add this as ghost peri
    'SYS',

    # These are software libraries
    'FREERTOS',
    'PDM2PCM',
    'FATFS',
    'LIBJPEG',
    'MBEDTLS',
    'LWIP',
    'USB_HOST',
    'USB_DEVICE',
    'GUI_INTERFACE',
    'TRACER_EMB',
    'TOUCHSENSING',
]

perimap = [
    ('.*:USART:sci2_v1_1', ('usart', 'v1', 'USART')),
    ('.*:USART:sci2_v1_2_F1', ('usart', 'v1', 'USART')),
    ('.*:USART:sci2_v1_2', ('usart', 'v1', 'USART')),
    ('.*:USART:sci2_v2_0', ('usart', 'v2', 'USART')),
    ('.*:USART:sci2_v2_1', ('usart', 'v2', 'USART')),
    ('.*:USART:sci2_v2_2', ('usart', 'v2', 'USART')),
    ('.*:USART:sci3_v1_0', ('usart', 'v2', 'USART')),
    ('.*:USART:sci3_v1_1', ('usart', 'v2', 'USART')),
    ('.*:USART:sci3_v1_2', ('usart', 'v2', 'USART')),
    ('.*:USART:sci3_v2_0', ('usart', 'v2', 'USART')),
    ('.*:USART:sci3_v2_1', ('usart', 'v2', 'USART')),
    ('.*:UART:sci2_v2_1', ('usart', 'v2', 'USART')),
    ('.*:UART:sci2_v3_0', ('usart', 'v2', 'USART')),
    ('.*:UART:sci2_v3_1', ('usart', 'v2', 'USART')),
    ('.*:LPUART:sci3_v1_1', ('lpuart', 'v1', 'LPUART')),
    ('.*:LPUART:sci3_v1_2', ('lpuart', 'v2', 'LPUART')),
    ('.*:LPUART:sci3_v1_3', ('lpuart', 'v2', 'LPUART')),
    ('.*:LPUART:sci3_v1_4', ('lpuart', 'v2', 'LPUART')),
    ('.*:RNG:rng1_v1_1', ('rng', 'v1', 'RNG')),
    ('.*:RNG:rng1_v2_0', ('rng', 'v1', 'RNG')),
    ('.*:RNG:rng1_v2_1', ('rng', 'v1', 'RNG')),
    ('.*:RNG:rng1_v3_1', ('rng', 'v1', 'RNG')),
    ('.*:SPI:spi2_v1_4', ('spi', 'f1', 'SPI')),
    ('.*:SPI:spi2s1_v2_2', ('spi', 'v1', 'SPI')),
    ('.*:SPI:spi2s1_v3_2', ('spi', 'v2', 'SPI')),
    ('.*:SPI:spi2s1_v3_3', ('spi', 'v2', 'SPI')),
    ('.*:SPI:spi2s1_v3_5', ('spi', 'v2', 'SPI')),
    ('.*:SUBGHZSPI:.*', ('spi', 'v2', 'SPI')),
    ('.*:SPI:spi2s1_v3_1', ('spi', 'v2', 'SPI')),
    ('.*:SPI:spi2s2_v1_1', ('spi', 'v3', 'SPI')),
    ('.*:SPI:spi2s2_v1_0', ('spi', 'v3', 'SPI')),
    ('.*:I2C:i2c1_v1_5', ('i2c', 'v1', 'I2C')),
    ('.*:I2C:i2c2_v1_1', ('i2c', 'v2', 'I2C')),
    ('.*:I2C:i2c2_v1_1F7', ('i2c', 'v2', 'I2C')),
    ('.*:I2C:i2c2_v1_1U5', ('i2c', 'v2', 'I2C')),
    ('.*:DAC:dacif_v1_1', ('dac', 'v1', 'DAC')),
    ('.*:DAC:dacif_v2_0', ('dac', 'v2', 'DAC')),
    ('.*:DAC:dacif_v3_0', ('dac', 'v2', 'DAC')),
    ('.*:ADC:aditf_v2_5F1', ('adc', 'f1', 'ADC')),
    ('.*:ADC:aditf2_v1_1', ('adc', 'v2', 'ADC')),
    ('.*:ADC:aditf5_v2_0', ('adc', 'v3', 'ADC')),
    ('STM32G0.*:ADC:.*', ('adc', 'g0', 'ADC')),
    ('STM32G0.*:ADC_COMMON:.*', ('adccommon', 'v3', 'ADC_COMMON')),
    ('.*:ADC_COMMON:aditf2_v1_1', ('adccommon', 'v2', 'ADC_COMMON')),
    ('.*:ADC_COMMON:aditf5_v2_0', ('adccommon', 'v3', 'ADC_COMMON')),
    ('.*:ADC_COMMON:aditf4_v3_0_WL', ('adccommon', 'v3', 'ADC_COMMON')),
    ('.*:DCMI:.*', ('dcmi', 'v1', 'DCMI')),
    ('STM32F0.*:SYSCFG:.*',  ('syscfg', 'f0', 'SYSCFG')),
    ('STM32F3.*:SYSCFG:.*',  ('syscfg', 'f3', 'SYSCFG')),
    ('STM32F4.*:SYSCFG:.*',  ('syscfg', 'f4', 'SYSCFG')),
    ('STM32F7.*:SYSCFG:.*',  ('syscfg', 'f7', 'SYSCFG')),
    ('STM32L4.*:SYSCFG:.*',  ('syscfg', 'l4', 'SYSCFG')),
    ('STM32L0.*:SYSCFG:.*',  ('syscfg', 'l0', 'SYSCFG')),
    ('STM32L1.*:SYSCFG:.*',  ('syscfg', 'l1', 'SYSCFG')),
    ('STM32G0.*:SYSCFG:.*',  ('syscfg', 'g0', 'SYSCFG')),
    ('STM32G4.*:SYSCFG:.*',  ('syscfg', 'g4', 'SYSCFG')),
    ('STM32H7.*:SYSCFG:.*',  ('syscfg', 'h7', 'SYSCFG')),
    ('STM32U5.*:SYSCFG:.*',  ('syscfg', 'u5', 'SYSCFG')),
    ('STM32WB.*:SYSCFG:.*',  ('syscfg', 'wb', 'SYSCFG')),
    ('STM32WL5.*:SYSCFG:.*', ('syscfg', 'wl5', 'SYSCFG')),
    ('STM32WLE.*:SYSCFG:.*', ('syscfg', 'wle', 'SYSCFG')),

    ('.*:IWDG:iwdg1_v2_0', ('iwdg', 'v2', 'IWDG')),
    ('.*:WWDG:wwdg1_v1_0', ('wwdg', 'v1', 'WWDG')),
    ('.*:JPEG:jpeg1_v1_0', ('jpeg', 'v1', 'JPEG')),
    ('.*:LPTIM:F7_lptimer1_v1_1', ('lptim', 'v1', 'LPTIM')),
    ('.*:LTDC:lcdtft1_v1_1', ('ltdc', 'v1', 'LTDC')),
    ('.*:MDIOS:mdios1_v1_0', ('mdios', 'v1', 'MDIOS')),
    ('.*:QUADSPI:quadspi1_v1_0', ('quadspi', 'v1', 'QUADSPI')),
    ('.*:RTC:rtc2_v2_6', ('rtc', 'v2', 'RTC')),
    ('.*:RTC:rtc2_v2_WB', ('rtc', 'wb', 'RTC')),
    ('.*:SAI:sai1_v1_1', ('sai', 'v1', 'SAI')),
    ('.*:SDMMC:sdmmc_v1_3', ('sdmmc', 'v1', 'SDMMC')),
    ('.*:SPDIFRX:spdifrx1_v1_0', ('spdifrx', 'v1', 'SPDIFRX')),
    ('.*:USB_OTG_FS:otgfs1_v1_2', ('otgfs', 'v1', 'OTG_FS')),
    ('.*:USB_OTG_HS:otghs1_v1_1', ('otghs', 'v1', 'OTG_HS')),

    ('STM32F0.0.*:RCC:.*', ('rcc', 'f0x0', 'RCC')),
    ('STM32F0.*:RCC:.*', ('rcc', 'f0', 'RCC')),
    ('STM32F1.*:RCC:.*', ('rcc', 'f1', 'RCC')),
    ('STM32F2.*:RCC:.*', ('rcc', 'f2', 'RCC')),
    ('STM32F3.*:RCC:.*', ('rcc', 'f3', 'RCC')),
    ('STM32F410.*:RCC:.*', ('rcc', 'f410', 'RCC')),
    ('STM32F4.*:RCC:.*', ('rcc', 'f4', 'RCC')),
    ('STM32F7.*:RCC:.*', ('rcc', 'f7', 'RCC')),
    ('STM32G0.*:RCC:.*', ('rcc', 'g0', 'RCC')),
    ('STM32G4.*:RCC:.*', ('rcc', 'g4', 'RCC')),
    ('STM32H7[AB].*:RCC:.*', ('rcc', 'h7ab', 'RCC')),
    ('STM32H7.*:RCC:.*', ('rcc', 'h7', 'RCC')),
    ('STM32L0.*:RCC:.*', ('rcc', 'l0', 'RCC')),
    ('STM32L1.*:RCC:.*', ('rcc', 'l1', 'RCC')),
    ('STM32L4.*:RCC:.*', ('rcc', 'l4', 'RCC')),
    ('STM32L5.*:RCC:.*', ('rcc', 'l5', 'RCC')),
    ('STM32U5.*:RCC:.*', ('rcc', 'u5', 'RCC')),
    ('STM32WB.*:RCC:.*', ('rcc', 'wb', 'RCC')),
    ('STM32WL5.*:RCC:.*', ('rcc', 'wl5', 'RCC')),
    ('STM32WLE.*:RCC:.*', ('rcc', 'wle', 'RCC')),

    ('STM32F3.*:SPI[1234]:.*', ('spi', 'v2', 'SPI')),

    ('STM32F1.*:AFIO:.*', ('afio', 'f1', 'AFIO')),

    ('STM32L5.*:EXTI:.*', ('exti', 'l5', 'EXTI')),
    ('STM32G0.*:EXTI:.*', ('exti', 'g0', 'EXTI')),
    ('STM32H7.*:EXTI:.*', ('exti', 'h7', 'EXTI')),
    ('STM32U5.*:EXTI:.*', ('exti', 'u5', 'EXTI')),
    ('STM32WB.*:EXTI:.*', ('exti', 'w', 'EXTI')),
    ('STM32WL5.*:EXTI:.*', ('exti', 'w', 'EXTI')),
    ('STM32WLE.*:EXTI:.*', ('exti', 'wle', 'EXTI')),
    ('.*:EXTI:.*', ('exti', 'v1', 'EXTI')),

    ('STM32L0.*:CRS:.*', ('crs', 'l0', 'CRS')),
    ('.*SDMMC:sdmmc2_v1_0', ('sdmmc', 'v2', 'SDMMC')),
    ('STM32G0.*:PWR:.*', ('pwr', 'g0', 'PWR')),
    ('STM32G4.*:PWR:.*', ('pwr', 'g4', 'PWR')),
    ('STM32H7(42|43|53|50).*:PWR:.*', ('pwr', 'h7', 'PWR')),
    ('STM32H7.*:PWR:.*', ('pwr', 'h7smps', 'PWR')),
    ('STM32F3.*:PWR:.*', ('pwr', 'f3', 'PWR')),
    ('STM32F4.*:PWR:.*', ('pwr', 'f4', 'PWR')),
    ('STM32F7.*:PWR:.*', ('pwr', 'f7', 'PWR')),
    ('STM32L1.*:PWR:.*', ('pwr', 'l1', 'PWR')),
    ('STM32U5.*:PWR:.*', ('pwr', 'u5', 'PWR')),
    ('STM32WL.*:PWR:.*', ('pwr', 'wl5', 'PWR')),
    ('STM32WB.*:PWR:.*', ('pwr', 'wb55', 'PWR')),
    ('STM32H7.*:FLASH:.*', ('flash', 'h7', 'FLASH')),
    ('STM32F0.*:FLASH:.*', ('flash', 'f0', 'FLASH')),
    ('STM32F1.*:FLASH:.*', ('flash', 'f1', 'FLASH')),
    ('STM32F3.*:FLASH:.*', ('flash', 'f3', 'FLASH')),
    ('STM32F4.*:FLASH:.*', ('flash', 'f4', 'FLASH')),
    ('STM32F7.*:FLASH:.*', ('flash', 'f7', 'FLASH')),
    ('STM32L1.*:FLASH:.*', ('flash', 'l1', 'FLASH')),
    ('STM32L4.*:FLASH:.*', ('flash', 'l4', 'FLASH')),
    ('STM32U5.*:FLASH:.*', ('flash', 'u5', 'FLASH')),
    ('STM32WB.*:FLASH:.*', ('flash', 'wb55', 'FLASH')),
    ('STM32F7.*:ETH:ETH:ethermac110_v2_0', ('eth', 'v1c', 'ETH')),
    ('.*ETH:ethermac110_v3_0', ('eth', 'v2', 'ETH')),

    ('STM32H7.*:FMC:.*', ('fmc', 'h7', 'FMC')),

    ('.*LPTIM\d.*:G0xx_lptimer1_v1_4', ('lptim', 'g0', 'LPTIM')),

    ('STM32H7.*:TIM1:.*', ('timer', 'v1', 'TIM_ADV')),
    ('STM32H7.*:TIM2:.*', ('timer', 'v1', 'TIM_GP32')),
    ('STM32H7.*:TIM5:.*', ('timer', 'v1', 'TIM_GP32')),
    ('STM32H7.*:TIM6:.*', ('timer', 'v1', 'TIM_BASIC')),
    ('STM32H7.*:TIM7:.*', ('timer', 'v1', 'TIM_BASIC')),
    ('STM32H7.*:TIM8:.*', ('timer', 'v1', 'TIM_ADV')),

    ('STM32F3.*:TIM(6|7){1}:.*', ('timer', 'v1', 'TIM_BASIC')),
    ('STM32F3.*:TIM(3|4|15|16|17){1}:.*', ('timer', 'v1', 'TIM_GP16')),
    ('STM32F3.*:TIM2:.*', ('timer', 'v1', 'TIM_GP32')),
    ('STM32F3.*:TIM(1|8|20){1}:.*', ('timer', 'v1', 'TIM_ADV')),

    ('STM32F7.*:TIM1:.*', ('timer', 'v1', 'TIM_ADV')),
    ('STM32F7.*:TIM8:.*', ('timer', 'v1', 'TIM_ADV')),
    ('.*TIM\d.*:gptimer.*', ('timer', 'v1', 'TIM_GP16')),

    ('STM32F0.*:DBGMCU:.*', ('dbgmcu', 'f0', 'DBGMCU')),
    ('STM32F1.*:DBGMCU:.*', ('dbgmcu', 'f1', 'DBGMCU')),
    ('STM32F2.*:DBGMCU:.*', ('dbgmcu', 'f2', 'DBGMCU')),
    ('STM32F3.*:DBGMCU:.*', ('dbgmcu', 'f3', 'DBGMCU')),
    ('STM32F4.*:DBGMCU:.*', ('dbgmcu', 'f4', 'DBGMCU')),
    ('STM32F7.*:DBGMCU:.*', ('dbgmcu', 'f7', 'DBGMCU')),
    ('STM32G0.*:DBGMCU:.*', ('dbgmcu', 'g0', 'DBGMCU')),
    ('STM32G4.*:DBGMCU:.*', ('dbgmcu', 'g4', 'DBGMCU')),
    ('STM32H7.*:DBGMCU:.*', ('dbgmcu', 'h7', 'DBGMCU')),
    ('STM32L0.*:DBGMCU:.*', ('dbgmcu', 'l0', 'DBGMCU')),
    ('STM32L1.*:DBGMCU:.*', ('dbgmcu', 'l1', 'DBGMCU')),
    ('STM32L4.*:DBGMCU:.*', ('dbgmcu', 'l4', 'DBGMCU')),
    ('STM32U5.*:DBGMCU:.*', ('dbgmcu', 'u5', 'DBGMCU')),
    ('STM32WB.*:DBGMCU:.*', ('dbgmcu', 'wb', 'DBGMCU')),
    ('STM32WL.*:DBGMCU:.*', ('dbgmcu', 'wl', 'DBGMCU')),

    ('STM32F1.*:GPIO.*', ('gpio', 'v1', 'GPIO')),
    ('.*:GPIO.*', ('gpio', 'v2', 'GPIO')),

    ('.*:IPCC:v1_0', ('ipcc', 'v1', 'IPCC')),
    ('.*:DMAMUX.*', ('dmamux', 'v1', 'DMAMUX')),

    ('.*:BDMA:.*', ('bdma', 'v1', 'DMA')),
    ('STM32H7.*:DMA2D:DMA2D:dma2d1_v1_0', ('dma2d', 'v2', 'DMA2D')),
    ('.*:DMA2D:dma2d1_v1_0', ('dma2d', 'v1', 'DMA2D')),
    ('STM32L4[PQRS].*:DMA.*', ('bdma', 'v1', 'DMA')),  # L4+
    ('STM32L[04].*:DMA.*', ('bdma', 'v2', 'DMA')),  # L0, L4 non-plus (since plus is handled above)
    ('STM32F030.C.*:DMA.*', ('bdma', 'v2', 'DMA')),  # Weird F0
    ('STM32F09.*:DMA.*', ('bdma', 'v2', 'DMA')),  # Weird F0
    ('STM32F[247].*:DMA.*', ('dma', 'v2', 'DMA')),
    ('STM32H7.*:DMA.*', ('dma', 'v1', 'DMA')),
    ('.*:DMA.*', ('bdma', 'v1', 'DMA')),

    ('.*:CAN:bxcan1_v1_1.*', ('can', 'bxcan', 'CAN')),
    # stm32F4 CRC peripheral
    # ("STM32F4*:CRC:CRC:crc_f4")
    # v1: F1, F2, F4, L1
    # v2, adds INIT reg: F0
    # v3, adds POL reg: F3, F7, G0, G4, H7, L0, L4, L5, WB, WL
    ('.*:CRC:integtest1_v1_0', ('crc', 'v1', 'CRC')),
    ('STM32L[04].*:CRC:integtest1_v2_0', ('crc', 'v3', 'CRC')),
    ('.*:CRC:integtest1_v2_0', ('crc', 'v2', 'CRC')),
    ('.*:CRC:integtest1_v2_2', ('crc', 'v3', 'CRC')),
]

peri_rename = {
    'HDMI_CEC': 'CEC',
    'SUBGHZ': 'SUBGHZSPI',
}

ghost_peris = [
    'GPIOA', 'GPIOB', 'GPIOC', 'GPIOD', 'GPIOE', 'GPIOF', 'GPIOG', 'GPIOH', 'GPIOI', 'GPIOJ', 'GPIOK', 'GPIOL', 'GPIOM', 'GPION', 'GPIOO', 'GPIOP', 'GPIOQ', 'GPIOR', 'GPIOS', 'GPIOT',
    'DMA1', 'DMA2', 'BDMA', 'DMAMUX', 'DMAMUX1', 'DMAMUX2',
    'SYSCFG', 'EXTI', 'FLASH', 'DBGMCU', 'CRS', 'PWR', 'AFIO',
]

alt_peri_defines = {
    'DBGMCU': ['DBGMCU_BASE', 'DBG_BASE'],
    'FLASH': ['FLASH_R_BASE', 'FLASH_REG_BASE'],
    'ADC_COMMON': ['ADC_COMMON', 'ADC1_COMMON', 'ADC12_COMMON', 'ADC123_COMMON'],
    'CAN': ['CAN_BASE', 'CAN1_BASE'],
    'FMC': ['FMC_BASE', 'FMC_R_BASE']
}

# Device address overrides, in case of missing from headers
address_overrides = {
    'STM32F412VE:GPIOF_BASE': 0x40021400,
    'STM32F412VE:GPIOG_BASE': 0x40021800,
    'STM32F412VG:GPIOF_BASE': 0x40021400,
    'STM32F412VG:GPIOG_BASE': 0x40021800,
    'STM32L151CB-A:GPIOF_BASE': 0x40021800,
    'STM32L151CB-A:GPIOG_BASE': 0x40021C00,
    'STM32L432KB:GPIOD_BASE': 0x48000C00,
    'STM32L432KB:GPIOE_BASE': 0x48001000,
    'STM32L432KB:GPIOF_BASE': 0x48001400,
    'STM32L432KB:GPIOG_BASE': 0x48001800,
}


def lookup_address(defines, name, d):
    if addr := defines.get(d):
        return addr
    elif addr := address_overrides.get(name + ':' + d):
        return addr


@functools.cache
def match_peri(peri):
    for r, block in perimap:
        if re.match('^' + r + '$', peri):
            if block == '':
                return None
            return block
    return None


all_mcu_files = {}
per_mcu_files = {}


def parse_documentations():
    print("linking files and documents")
    with open('sources/mcufinder/files.json', 'r', encoding='utf-8') as j:
        files = json.load(j)
        for file in files['Files']:
            file_id = file['id_file']
            if file_id not in all_mcu_files:
                all_mcu_files[file_id] = {
                    'name': file['name'],
                    'title': file['title'],
                    'url': file['URL'],
                    'type': file['type'],
                }

    with open('sources/mcufinder/mcus.json', 'r', encoding='utf-8') as j:
        mcus = json.load(j)
        for mcu in mcus['MCUs']:
            rpn = mcu['RPN']
            if rpn not in per_mcu_files:
                per_mcu_files[rpn] = []
            for file in mcu['files']:
                per_mcu_files[rpn].append(file['file_id'])


def parse_document_type(t):
    if t == 'Reference manual':
        return 0, 'reference_manual'
    if t == 'Programming manual':
        return 1, 'programming_manual'
    if t == 'Datasheet':
        return 2, 'datahseet'
    if t == 'Errata sheet':
        return 3, 'errata_sheet'
    if t == 'Application note':
        return 4, 'application_note'
    raise Exception(f'Unknown doc type {t}')


def documents_for(chip_name):
    docs = []
    if ids := per_mcu_files.get(chip_name):
        for id in ids:
            if file := all_mcu_files.get(id):
                file = all_mcu_files[id]
                order, doc_type = parse_document_type(file['type'])
                docs.append({
                    'order': order,
                    'type': doc_type,
                    'title': file['title'],
                    'name': file['name'],
                    'url': file['url'],
                })
    docs.sort(key=lambda x: (x['order'], x['name']))
    for doc in docs:
        del doc['order']
    return docs


def chip_name_from_package_name(x):
    name_map = [
        ('(STM32L1....).x([AX])', '\\1-\\2'),
        ('(STM32G0....).xN', '\\1'),
        ('(STM32F412..).xP', '\\1'),
        ('(STM32L4....).xP', '\\1'),
        ('(STM32WB....).x[AE]', '\\1'),
        ('(STM32G0....).xN', '\\1'),
        ('(STM32L5....).x[PQ]', '\\1'),
        ('(STM32L0....).xS', '\\1'),
        ('(STM32H7....).xQ', '\\1'),
        ('(STM32U5....).xQ', '\\1'),
        ('(STM32......).x', '\\1'),
    ]

    for a, b in name_map:
        r, n = re.subn('^' + a + '$', b, x)
        if n != 0:
            return r
    raise Exception("bad name: {}".format(x))


memories_map = {
    'flash': [
        'FLASH', 'FLASH_BANK1', 'FLASH_BANK2',
        'D1_AXIFLASH', 'D1_AXIICP',
    ],
    'ram': [
        'SRAM', 'SRAM1', 'SRAM2',
        'D1_AXISRAM',
        'D1_ITCMRAM',
        'D1_DTCMRAM',
        'D1_AHBSRAM',
        'D2_AXISRAM',
        'D3_BKPSRAM',
        'D3_SRAM'
    ],
}


def cleanup_pin_name(pin_name):
    if p := parse_pin_name(pin_name):
        return f'P{p[0]}{p[1]}'


def parse_signal_name(signal_name):
    if signal_name.startswith('USB_OTG_FS') or signal_name.startswith('USB_OTG_HS'):
        parts = [signal_name[:10], signal_name[11:]]
    else:
        parts = signal_name.split('_', 1)

    if len(parts) == 1:
        return None
    peri_name = parts[0]
    signal_name = parts[1]
    if signal_name.startswith("EXTI"):
        return None
    if peri_name.startswith("DEBUG") and signal_name.startswith("SUBGHZSPI"):
        parts = signal_name.split('-', 1)
        if len(parts) == 2:
            peri_name = parts[0]
            signal_name = removesuffix(parts[1], "OUT")

    return peri_name, signal_name


def parse_pin_name(pin_name):
    if len(pin_name) < 3:
        return None
    if pin_name[0] != 'P':
        return None
    port = pin_name[1]
    if not port.isalpha():
        return None

    pin = pin_name[2:]
    i = 0
    while i < len(pin) and pin[i].isnumeric():
        i += 1

    if i == 0:
        return None

    pin = int(pin[:i])

    return port, pin


def get_peri_addr(defines, pname):
    possible_defines = alt_peri_defines.get(pname) or [f'{pname}_BASE', pname]
    for d in possible_defines:
        if addr := defines.get(d):
            return addr
    return None


def parse_chips():
    os.makedirs('data/chips', exist_ok=True)

    # XMLs group together chips that are identical except flash/ram size.
    # For example STM32L471Z(E-G)Jx.xml is STM32L471ZEJx, STM32L471ZGJx.
    # However they do NOT group together identical chips with different package.
    #
    # We want exactly the opposite: group all packages of a chip together, but
    # NOT group equal-except-memory-size chips together. Yay.
    #
    # We first read all XMLs, and fold together all packages. We don't expand
    # flash/ram sizes yet, we want to do it as late as possible to avoid duplicate
    # work so that generation is faster.

    chips = {}
    chip_groups = []

    for f in sorted(glob('sources/cubedb/mcu/STM32*.xml')):
        f = f.replace(os.path.sep, '/')
        if 'STM32MP' in f:
            continue
        if 'STM32GBK' in f:
            continue

        print(f)

        r = xmltodict.parse(open(f, 'rb'), force_list=['Signal'])['Mcu']

        package_names = expand_name(r['@RefName'])
        package_rams = r['Ram']
        package_flashs = r['Flash']
        if type(package_rams) != list:
            package_rams = [package_rams] * len(package_names)
        if type(package_flashs) != list:
            package_flashs = [package_flashs] * len(package_names)

        group_idx = None
        for package_name in package_names:
            chip_name = chip_name_from_package_name(package_name)
            if chip := chips.get(chip_name):
                group_idx = chip['group_idx']
                break

        if group_idx is None:
            group_idx = len(chip_groups)
            chip_groups.append({
                'chip_names': [],
                'xml': r,
                'ips': {},
                'pins': {},
            })

        for package_i, package_name in enumerate(package_names):
            chip_name = chip_name_from_package_name(package_name)
            if chip_name not in chips:
                chips[chip_name] = {
                    'name': chip_name,
                    'flash': package_flashs[package_i],
                    'ram': package_rams[package_i],
                    'group_idx': group_idx,
                    'packages': [],
                }
            chips[chip_name]['packages'].append({
                'name': package_name,
                'package': r['@Package'],
            })

        # Some packages have some peripehrals removed because the package had to
        # remove GPIOs useful for that peripheral. So we merge all peripherals from all packages.
        group = chip_groups[group_idx]
        for ip in r['IP']:
            group['ips'][ip['@InstanceName']] = ip
        for pin in r['Pin']:
            if pin_name := cleanup_pin_name(pin['@Name']):
                group['pins'][pin_name] = pin

    for chip_name, chip in chips.items():
        chip_groups[chip['group_idx']]['chip_names'].append(chip_name)

    for chip in chip_groups:
        chip_name = chip["chip_names"][0]
        print(f'* processing chip group {chip["chip_names"]}')

        chip['family'] = chip['xml']['@Family']
        chip['line'] = chip['xml']['@Line']
        chip['die'] = chip['xml']['Die']

        chip_nvic = next(filter(lambda x: x['@Name'] == 'NVIC', chip['ips'].values()), None)
        # L5, U5 have NVIC1=S, NVIC2=NS. Use NS.
        if chip_nvic is None and not chip_name.startswith('STM32L5') and not chip_name.startswith('STM32U5'):
            chip_nvic = next(filter(lambda x: x['@Name'] == 'NVIC1', chip['ips'].values()), None)
        if chip_nvic is None:
            chip_nvic = next(filter(lambda x: x['@Name'] == 'NVIC2', chip['ips'].values()), None)

        chip_dma = next(filter(lambda x: x['@Name'] == 'DMA', chip['ips'].values()), None)
        if chip_dma is not None:
            chip_dma = chip_dma['@Version']

        chip_bdma = next(filter(lambda x: x['@Name'] == 'BDMA', chip['ips'].values()), None)
        if chip_bdma is not None:
            chip_bdma = chip_bdma['@Version']

        rcc_kind = next(filter(lambda x: x['@Name'] == 'RCC', chip['ips'].values()))['@Version']
        assert rcc_kind is not None
        rcc_block = match_peri(f'{chip_name}:RCC:{rcc_kind}')
        assert rcc_block is not None

        h = header.get_for_chip(chip_name)
        if h is None:
            raise Exception("missing header for {}".format(chip_name))

        chip_af = next(filter(lambda x: x['@Name'] == 'GPIO', chip['ips'].values()))['@Version']
        chip_af = removesuffix(chip_af, '_gpio_v1_0')
        chip_af = af[chip_af]

        cores = []
        for core_xml in children(chip['xml'], 'Core'):
            core_name = corename(core_xml)
            core = {
                'name': core_name,
                'peripherals': [],
            }
            cores.append(core)

            if not core_name in h['interrupts'] or not core_name in h['defines']:
                core_name = 'all'
            # print("Defining for core", core_name)

            # Gather all interrupts and defines for this core

            header_irqs = h['interrupts'][core_name]
            chip_irqs = interrupts.get(chip_nvic['@Name'], chip_nvic['@Version'], core_name)

            defines = h['defines'][core_name]

            # F100xE MISC_REMAP remaps some DMA IRQs, so ST decided to give two names
            # to the same IRQ number.
            if chip_name.startswith('STM32F100') and 'DMA2_Channel4_5' in header_irqs:
                del header_irqs['DMA2_Channel4_5']

            core['interrupts'] = [
                {
                    'name': k,
                    'number': v,
                }
                for k, v in header_irqs.items()
            ]

            peri_kinds = {}

            for ip in chip['ips'].values():
                pname = ip['@InstanceName']
                pkind = ip['@Name'] + ':' + ip['@Version']
                pkind = removesuffix(pkind, '_Cube')

                if pname in FAKE_PERIPHERALS:
                    continue

                if rename := peri_rename.get(pname):
                    pname = rename

                if pname.startswith('ADC'):
                    if not 'ADC_COMMON' in peri_kinds:
                        peri_kinds['ADC_COMMON'] = 'ADC_COMMON:' + removesuffix(ip['@Version'], '_Cube')

                peri_kinds[pname] = pkind

            for pname in ghost_peris:
                if pname not in peri_kinds and (addr := get_peri_addr(defines, pname)):
                    peri_kinds[pname] = 'unknown'

            # get possible used GPIOs for each peripheral from the chip xml
            # it's not the full info we would want (stuff like AFIO info which comes from GPIO xml),
            #   but we actually need to use it because of F1 line
            #       which doesn't include non-remappable peripherals in GPIO xml
            #   and some weird edge cases like STM32F030C6 (see merge_periph_pins_info)
            periph_pins = {}
            for pin_name, pin in chip['pins'].items():
                for signal in pin['Signal']:
                    signal = signal['@Name']
                    if signal.startswith('DEBUG_SUBGHZSPI-'):
                        signal = 'SUBGHZSPI_' + signal[16:-3]
                    # TODO: What are those signals (well, GPIO is clear) Which peripheral do they belong to?
                    if signal not in {'GPIO', 'CEC', 'AUDIOCLK', 'VDDTCXO'} and 'EXTI' not in signal:
                        # both peripherals and signals can have underscores in their names so there is no easy way to split
                        # check if signal name starts with one of the peripheral names
                        for periph in peri_kinds.keys():
                            if signal.startswith(periph + '_'):
                                signal = removeprefix(signal, periph + '_')
                                pins = periph_pins.setdefault(periph, [])
                                pins.append({
                                    'pin': pin_name,
                                    'signal': signal,
                                })
                                break
            for periph, pins in periph_pins.items():
                pins = remove_duplicates(pins)
                sort_pins(pins)
                periph_pins[periph] = pins

            peris = []
            for pname, pkind in peri_kinds.items():
                addr = get_peri_addr(defines, pname)
                if addr is None:
                    continue

                p = {
                    'name': pname,
                    'address': addr,
                }

                if block := match_peri(chip_name + ':' + pname + ':' + pkind):
                    p['registers'] = {
                        'kind': block[0],
                        'version': block[1],
                        'block': block[2],
                    }

                if rcc_info := match_peri_clock(rcc_block, pname):
                    p['rcc'] = rcc_info

                if pins := periph_pins.get(pname):
                    # merge the core xml info with GPIO xml info to hopefully get the full picture
                    # if the peripheral does not exist in the GPIO xml (one of the notable one is ADC)
                    #   it probably doesn't need any AFIO writes to work
                    if af_pins := chip_af.get(pname):
                        pins = merge_periph_pins_info('STM32F1' in chip_name, pname, pins, af_pins)
                    p['pins'] = pins

                if pname in chip_irqs:
                    # filter by available, because some are conditioned on <Die>
                    p['interrupts'] = interrupts.filter_interrupts(chip_irqs[pname], header_irqs)

                peris.append(p)

            family_extra = "data/extra/family/" + chip['family'] + ".yaml"
            if os.path.exists(family_extra):
                with open(family_extra) as extra_f:
                    extra = yaml.load(extra_f)
                    for p in extra['peripherals']:
                        peris.append(p)

            peris.sort(key=lambda x: x['name'])
            have_peris = set((p['name'] for p in peris))
            core['peripherals'] = peris

            # Process DMA channels
            chs = []
            if x := dma_channels.get(chip_dma):
                chs.extend(x['channels'])
            if x := dma_channels.get(chip_bdma):
                chs.extend(x['channels'])

            # The dma_channels[xx] is generic for multiple chips. The current chip may have less DMAs,
            # so we have to filter it.
            chs = [ch for ch in chs if ch['dma'] in have_peris]
            core['dma_channels'] = chs

            have_chs = set((ch['name'] for ch in chs))

            # Process peripheral - DMA channel associations
            if chip_dma is not None:
                for p in peris:
                    if peri_chs := dma_channels[chip_dma]['peripherals'].get(p['name']):
                        p['dma_channels'] = [
                            ch
                            for ch in peri_chs
                            if 'channel' not in ch or ch['channel'] in have_chs
                        ]

        # Now that we've processed everything common to the entire group,
        # process each chip in the group.

        group = chip

        for chip_name in group['chip_names']:
            chip = chips[chip_name]

            flash_total = int(chip['flash']) * 1024
            ram_total = int(chip['ram']) * 1024

            memory_regions = []

            found = set()
            for each in memories_map['flash']:
                if each + '_BASE' in h['defines']['all']:
                    if each == 'FLASH':
                        key = 'BANK_1'
                    elif each == 'FLASH_BANK1':
                        key = 'BANK_1'
                    elif each == 'FLASH_BANK2':
                        key = 'BANK_2'
                    else:
                        key = each

                    if key in found:
                        continue
                    found.add(key)

                    size = 0
                    if key == 'BANK_1' or key == 'BANK_2':
                        if size2 := memory.determine_flash_size(chip_name):
                            size = min(size2, flash_total)

                    memory_regions.append({
                        'name': key,
                        'kind': 'flash',
                        'address': h['defines']['all'][each + '_BASE'],
                        'size': size,
                    })

            found = set()
            for each in memories_map['ram']:
                if each + '_BASE' in h['defines']['all']:
                    if each == 'D1_AXISRAM':
                        key = 'SRAM'
                    elif each == 'SRAM1':
                        key = 'SRAM'
                    else:
                        key = each

                    if key in found:
                        continue
                    found.add(key)

                    size = 0
                    if key == 'SRAM':
                        if size2 := memory.determine_ram_size(chip_name):
                            size = min(size2, ram_total)

                    memory_regions.append({
                        'name': key,
                        'kind': 'ram',
                        'address': h['defines']['all'][each + '_BASE'],
                        'size': size,
                    })

            docs = documents_for(chip_name)

            device_id = memory.determine_device_id(chip_name)

            chip = {
                'name': chip_name,
                'family': group['family'],
                'line': group['line'],
                'die': group['die'],
                'device_id': device_id,
                'packages': chip['packages'],
                'memory': memory_regions,
                'docs': docs,
                'cores': cores,
            }

            with open('data/chips/' + chip_name + '.json', 'w') as f:
                json.dump(chip, f, indent=4)


SIGNAL_REMAP = {
    # for some godforsaken reason UART4's and UART5's CTS are called CTS_NSS in the GPIO xml
    # so try to match with these
    'CTS': 'CTS_NSS'


}


def merge_periph_pins_info(is_f1, periph_name, core_pins, af_pins):
    if is_f1:
        # TODO: actually handle the F1 AFIO information when it will be extracted
        return core_pins

    # covert to dict
    af_pins = {(v['pin'], v['signal']): v for v in af_pins}
    for pin in core_pins:
        af = af_pins.get((pin['pin'], pin['signal']), {'af': None})['af']

        # try to look for a signal with another name
        if af is None and (remap := SIGNAL_REMAP.get(pin['signal'])):
            af = af_pins.get((pin['pin'], remap), {'af': None})['af']

        # it appears that for __some__ STM32 MCUs there is no AFIO specified in GPIO file
        # (notably - STM32F030C6 with it's I2C1 on PF6 and PF7)
        # but the peripheral can actually be mapped to different pins
        # this breaks embassy's model, so we pretend that it's AF 0
        # Reference Manual states that there's no GPIOF_AFR register
        # but according to Cube-generated core it's OK to write to AFIO reg, it seems to be ignored
        # TODO: are there any more signals that have this "feature"
        if af is None and periph_name == 'I2C1':
            af = 0

        if af is not None:
            # Kinda not nice to modify this dict, but doesn't seem to be a problem
            pin['af'] = af
    return core_pins
    pass


af = {}


def sort_pins(pins):
    pins.sort(key=lambda p: (parse_pin_name(p['pin']), p['signal']))


def remove_duplicates(item_list):
    ''' Removes duplicate items from a list '''
    singles_list = []
    for element in item_list:
        if element not in singles_list:
            singles_list.append(element)
    return singles_list


def parse_gpio_af():
    # os.makedirs('data/gpio_af', exist_ok=True)
    for f in glob('sources/cubedb/mcu/IP/GPIO-*_gpio_v1_0_Modes.xml'):
        f = f.replace(os.path.sep, '/')

        ff = removeprefix(f, 'sources/cubedb/mcu/IP/GPIO-')
        ff = removesuffix(ff, '_gpio_v1_0_Modes.xml')
        r = xmltodict.parse(open(f, 'rb'))

        if 'STM32F1' in f:
            peris = parse_gpio_af_f1(r)
        else:
            peris = parse_gpio_af_nonf1(r)
        af[ff] = peris


def parse_gpio_af_f1(xml):
    peris = {}
    for pin in xml['IP']['GPIO_Pin']:
        pin_name = pin['@Name']

        # Cleanup pin name
        pin_name = cleanup_pin_name(pin_name)
        if pin_name is None:
            continue

        # Extract AFs
        for signal in children(pin, 'PinSignal'):
            p = parse_signal_name(signal['@Name'])
            if p is None:
                continue
            peri_name, signal_name = p

            if peri_name not in peris:
                peris[peri_name] = []
            peris[peri_name].append({
                'pin': pin_name,
                'signal': signal_name,
            })

    for pname, p in peris.items():
        p = remove_duplicates(p)
        sort_pins(p)
        peris[pname] = p
    return peris


def parse_gpio_af_nonf1(xml):
    peris = {}

    for pin in xml['IP']['GPIO_Pin']:
        pin_name = pin['@Name']

        # Cleanup pin name
        pin_name = cleanup_pin_name(pin_name)
        if pin_name is None:
            continue

        # Extract AFs
        for signal in children(pin, 'PinSignal'):
            p = parse_signal_name(signal['@Name'])
            if p is None:
                continue
            peri_name, signal_name = p

            afn = signal['SpecificParameter']['PossibleValue'].split('_')[1]
            afn = int(removeprefix(afn, 'AF'))

            if peri_name not in peris:
                peris[peri_name] = []
            peris[peri_name].append({
                'pin': pin_name,
                'signal': signal_name,
                'af': afn,
            })

    for pname, p in peris.items():
        p = remove_duplicates(p)
        sort_pins(p)
        peris[pname] = p
    return peris


dma_channels = {}


def parse_dma():
    for f in glob('sources/cubedb/mcu/IP/*DMA-*Modes.xml'):
        f = f.replace(os.path.sep, '/')
        is_explicitly_bdma = False
        ff = removeprefix(f, 'sources/cubedb/mcu/IP/')
        if not (ff.startswith('B') or ff.startswith('D')):
            continue
        if ff.startswith("BDMA"):
            is_explicitly_bdma = True
        ff = removeprefix(ff, 'DMA-')
        ff = removeprefix(ff, 'BDMA-')
        ff = removesuffix(ff, '_Modes.xml')

        r = xmltodict.parse(open(f, 'rb'), force_list={'Mode', 'RefMode'})

        chip_dma = {
            'channels': [],
            'peripherals': {},
        }

        for dma in r['IP']['ModeLogicOperator']['Mode']:
            dma_peri_name = dma['@Name']
            if ' Context' in dma_peri_name:
                continue
            channels = dma['ModeLogicOperator']['Mode']
            if len(channels) == 1:
                # ========== CHIP WITH DMAMUX

                dmamux_file = ff[5:7]
                if ff.startswith('STM32L4P'):
                    dmamux_file = 'L4PQ'
                if ff.startswith('STM32L4S'):
                    dmamux_file = 'L4RS'
                for mf in sorted(glob('data/dmamux/{}_*.yaml'.format(dmamux_file))):
                    mf = mf.replace(os.path.sep, '/')
                    with open(mf, 'r') as yaml_file:
                        y = yaml.load(yaml_file)
                    mf = removesuffix(mf, '.yaml')
                    dmamux = mf[mf.index('_') + 1:]  # DMAMUX1 or DMAMUX2

                    for (request_name, request_num) in y.items():
                        parts = request_name.split('_')
                        target_peri_name = parts[0]
                        if len(parts) < 2:
                            request = target_peri_name
                        else:
                            request = parts[1]
                        chip_dma['peripherals'].setdefault(target_peri_name, []).append({
                            'signal': request,
                            "dmamux": dmamux,
                            "request": request_num,
                        })

                dmamux = 'DMAMUX1'
                if is_explicitly_bdma:
                    dmamux = 'DMAMUX2'

                dmamux_channel = 0
                for n in dma_peri_name.split(","):
                    n = n.strip()
                    if result := re.match('.*' + n + '_(Channel|Stream)\[(\d+)-(\d+)\]', channels[0]['@Name']):
                        low = int(result.group(2))
                        high = int(result.group(3))
                        # Make sure all channels numbers start at 0
                        if low == 1:
                            low -= 1
                            high -= 1
                        for i in range(low, high + 1):
                            chip_dma['channels'].append({
                                'name': n + '_CH' + str(i),
                                'dma': n,
                                'channel': i,
                                'dmamux': dmamux,
                                'dmamux_channel': dmamux_channel,
                            })
                            dmamux_channel += 1

            else:
                # ========== CHIP WITHOUT DMAMUX

                # see if we can scrape out requests
                requests = {}

                request_blocks = filter(lambda x: x['@BaseMode'] == 'DMA_Request', r['IP']['RefMode'])
                for block in request_blocks:
                    name = block['@Name']
                    # Depending on the chip, the naming is "Channel" or "Request"...
                    request_num = next(filter(lambda x: x['@Name'] in ('Channel', 'Request'), block['Parameter']), None)
                    if request_num is not None:
                        request_num = request_num['PossibleValue']
                        request_num = removeprefix(request_num, "DMA_CHANNEL_")
                        request_num = removeprefix(request_num, "DMA_REQUEST_")
                        requests[name] = int(request_num)

                channel_names = []
                for channel in channels:
                    channel_name = channel['@Name']
                    channel_name = removeprefix(channel_name, dma_peri_name + '_')
                    channel_name = removeprefix(channel_name, "Channel")
                    channel_name = removeprefix(channel_name, "Stream")

                    channel_names.append(channel_name)
                    chip_dma['channels'].append({
                        'name': dma_peri_name + '_CH' + channel_name,
                        'dma': dma_peri_name,
                        'channel': int(channel_name),
                    })
                    for target in channel['ModeLogicOperator']['Mode']:
                        target_name = target['@Name']
                        original_target_name = target_name
                        parts = target_name.split(':')
                        target_name = parts[0]
                        parts = target_name.split('_')
                        target_peri_name = parts[0]
                        if len(parts) < 2:
                            target_requests = [target_peri_name]
                        else:
                            target_requests = target_name.split('_')[1].split('/')
                        if target_name != 'MEMTOMEM':
                            if target_peri_name == "LPUART":
                                target_peri_name = "LPUART1"
                            for request in target_requests:
                                if ':' in request:
                                    request = request.split(':')[0]
                                entry = {
                                    'signal': request,
                                    'channel': dma_peri_name + '_CH' + channel_name,
                                }
                                if original_target_name in requests:
                                    entry['request'] = requests[original_target_name]
                                chip_dma['peripherals'].setdefault(target_peri_name, []).append(entry)

                # Make sure all channels numbers start at 0
                if min(map(int, channel_names)) != 0:
                    for ch in chip_dma['channels']:
                        if ch['dma'] == dma_peri_name:
                            ch['channel'] -= 1

        dma_channels[ff] = chip_dma


peripheral_to_clock = {}


clock_renames = {
    'AHB': 'AHB1',
    'APB': 'APB1',
}


def parse_rcc_regs():
    print("parsing RCC registers")
    for f in glob('data/registers/rcc_*'):
        f = f.replace(os.path.sep, '/')
        ff = removeprefix(f, 'data/registers/rcc_')
        ff = removesuffix(ff, '.yaml')
        family_clocks = {}
        with open(f, 'r') as yaml_file:
            y = yaml.load(yaml_file)

        for (key, body) in y.items():
            # Some chip families have a separate bus for GPIO so it's not attached to the AHB/APB
            # bus but an GPIO bus. Use the GPIO as the clock for these chips.
            if m := re.match('^fieldset/((A[PH]B\d?)|GPIO)[LH]?ENR\d?$', key):
                reg = removeprefix(key, 'fieldset/')
                clock = m.group(1)
                clock = clock_renames.get(clock, clock)
                for field in body['fields']:
                    if field['name'].endswith('EN'):
                        peri = removesuffix(field['name'], 'EN')

                        res = {
                            'clock': clock,
                            'enable': {
                                'register': reg,
                                'field': field['name'],
                            }
                        }
                        if rstr := y[key.replace('ENR', 'RSTR')]:
                            if field := next(filter(lambda f: f['name'] == f'{peri}RST', rstr['fields']), None):
                                res['reset'] = {
                                    'register': reg.replace('ENR', 'RSTR'),
                                    'field': f'{peri}RST',
                                }
                        family_clocks[peri] = res

        peripheral_to_clock[('rcc', ff, 'RCC')] = family_clocks


def match_peri_clock(rcc_block, peri_name):
    if rcc_block in peripheral_to_clock:
        if res := peripheral_to_clock[rcc_block].get(peri_name):
            return res
        if peri_name.endswith("1"):
            return match_peri_clock(rcc_block, removesuffix(peri_name, "1"))
        return None


memory.parse()
interrupts.parse()
parse_rcc_regs()
parse_documentations()
parse_dma()
parse_gpio_af()
parse_chips()
