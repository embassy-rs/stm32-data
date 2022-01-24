from stm32data.util import *
from glob import glob
import xmltodict
import re
import os


chip_interrupts = {}


def get(nvic_name, nvic_version, core):
    return chip_interrupts[(nvic_name, nvic_version)]


def parse():
    print("parsing interrupts")
    for f in glob('sources/cubedb/mcu/IP/NVIC*_Modes.xml'):
        f = f.replace(os.path.sep, '/')
        ff = removeprefix(f, 'sources/cubedb/mcu/IP/')
        ff = removesuffix(ff, '_Modes.xml')

        [nvic_name, nvic_version] = ff.split('-')

        chip_irqs = {}
        r = xmltodict.parse(open(f, 'rb'))

        irqs = next(filter(lambda x: x['@Name'] == 'IRQn', r['IP']['RefParameter']))
        for irq in irqs['PossibleValue']:
            value = irq['@Value']
            parts = value.split(':')
            irq_name = removesuffix(parts[0], "_IRQn")

            # F100xE MISC_REMAP remaps some DMA IRQs, so ST decided to give two names
            # to the same IRQ number.
            if nvic_version == 'STM32F100E' and irq_name == 'DMA2_Channel4_5':
                continue

            peri_names = parts[2].split(',')
            if len(peri_names) == 1 and peri_names[0] == '':
                continue
            elif len(peri_names) == 1 and (peri_names[0] == 'DMA' or peri_names[0].startswith("DMAL")):
                peri_names = [parts[3]]
            split = split_interrupts(peri_names, irq_name)
            for p in peri_names:
                if p not in chip_irqs:
                    chip_irqs[p] = {}
                merge_peri_irq_signals(chip_irqs[p], split[p])
        chip_interrupts[(nvic_name, nvic_version)] = chip_irqs


def merge_peri_irq_signals(peri_irqs, additional):
    for key, value in additional.items():
        if key not in peri_irqs:
            peri_irqs[key] = []
        peri_irqs[key].append(value)


def split_interrupts(peri_names, irq_name):
    split = {}
    for p in peri_names:
        split[p] = remap_interrupt_signals(p, irq_name)

    return split


irq_signals_map = {
    'CAN': ['TX', 'RX0', 'RX1', 'SCE'],
    'I2C': ['ER', 'EV'],
    'TIM': ['BRK', 'UP', 'TRG', 'COM'],
    'HRTIM': ['Master', 'TIMA', 'TIMB', 'TIMC', 'TIMD', 'TIME', 'TIMF']
}


def remap_interrupt_signals(peri_name, irq_name):
    if peri_name == irq_name:
        return expand_all_irq_signals(peri_name, irq_name)
    if (peri_name.startswith('DMA') or peri_name.startswith('BDMA')) and irq_name.startswith(peri_name):
        return {irq_name: irq_name}
    if peri_name.startswith('USART') and irq_name.startswith(peri_name):
        return {'GLOBAL': irq_name}
    if peri_name in irq_name:
        signals = {}
        start = irq_name.index(peri_name)
        regexp = re.compile('(_[^_]+)')
        if match := regexp.findall(irq_name, start):
            for m in match:
                signal = removeprefix(m, '_').strip()
                if is_valid_irq_signal(peri_name, signal):
                    signals[signal] = irq_name
        else:
            signals = expand_all_irq_signals(peri_name, irq_name)
        return signals
    else:
        return {'GLOBAL': irq_name}


def is_valid_irq_signal(peri_name, signal):
    for prefix, signals in irq_signals_map.items():
        if peri_name.startswith(prefix):
            return signal in signals
    return False


def expand_all_irq_signals(peri_name, irq_name):
    expanded = {}
    for prefix, signals in irq_signals_map.items():
        if peri_name.startswith(prefix):
            for s in irq_signals_map[prefix]:
                expanded[s] = irq_name
            return expanded

    return {'GLOBAL': irq_name}


def filter_interrupts(peri_irqs, all_irqs):
    filtered = {}

    for signal, irqs in peri_irqs.items():
        for irq in all_irqs:
            if irq in irqs:
                filtered[signal] = irq
                break

    return filtered
