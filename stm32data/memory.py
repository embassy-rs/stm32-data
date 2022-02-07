import sys
import re
import xmltodict
from glob import glob
from stm32data.util import *


def splat_names(base, parts):
    names = []
    for part in parts:
        if part.startswith("STM32"):
            names.append(base)
        elif part.startswith(base[5]):
            names.append('STM32' + part)
        else:
            names.append(base[0: len(base) - len(part)] + part)

    return names


def split_names(str):
    cleaned = []
    names = str.split("/")
    current_base = None
    for name in names:
        name = name.split(' ')[0].strip()
        if '-' in name:
            parts = name.split('-')
            current_base = parts[0]
            splatted = splat_names(current_base, parts)
            current_base = splatted[0]
            cleaned = cleaned + splatted
        elif name.startswith("STM32"):
            current_base = name
            cleaned.append(name)
        elif name.startswith(current_base[5]):
            names.append('STM32' + name)
        else:
            cleaned.append(current_base[0: len(current_base) - len(name)] + name)
    return cleaned


memories = []


def parse():
    for f in sorted(glob('sources/cubeprogdb/db/*.xml')):
        #print("parsing ", f);
        device = xmltodict.parse(open(f, 'rb'))['Root']['Device']
        device_id = device['DeviceID']
        name = device['Name']
        names = split_names(name)
        flash_size = None
        flash_addr = None
        ram_size = None
        ram_addr = None

        for peripheral in device['Peripherals']['Peripheral']:
            if peripheral['Name'] == 'Embedded SRAM' and ram_size is None:
                configs = peripheral['Configuration']
                if type(configs) != list:
                    configs = [configs]
                ram_addr = int(configs[0]['Parameters']['@address'], 16)
                ram_size = int(configs[0]['Parameters']['@size'], 16)
                #print( f'ram {addr} {size}')
            if peripheral['Name'] == 'Embedded Flash' and flash_size is None:
                configs = peripheral['Configuration']
                if type(configs) != list:
                    configs = [configs]
                flash_addr = int(configs[0]['Parameters']['@address'], 16)
                flash_size = int(configs[0]['Parameters']['@size'], 16)
                #print( f'flash {addr} {size}')

        chunk = {
            'device-id': int(device_id, 16),
            'names': names,
        }

        if ram_size is not None:
            chunk['ram'] = {
                'address': ram_addr,
                'bytes': ram_size,
            }

        if flash_size is not None:
            chunk['flash'] = {
                'address': flash_addr,
                'bytes': flash_size,
            }

        memories.append(chunk)


def determine_ram_size(chip_name):
    for each in memories:
        for name in each['names']:
            if is_chip_name_match(name, chip_name):
                return each['ram']['bytes']

    return None


def determine_flash_size(chip_name):
    for each in memories:
        for name in each['names']:
            if is_chip_name_match(name, chip_name):
                return each['flash']['bytes']

    return None


def determine_device_id(chip_name):
    for each in memories:
        for name in each['names']:
            if is_chip_name_match(name, chip_name):
                return each['device-id']
    return None


def is_chip_name_match(pattern, chip_name):
    pattern = pattern.replace('x', '.')
    return re.match(pattern + ".*", chip_name)
