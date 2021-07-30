#!/usr/bin/env python3

import sys
import xmltodict
import yaml
from collections import OrderedDict
from glob import glob

try:
    from yaml import CSafeLoader as SafeLoader
except ImportError:
    from yaml import SafeLoader


def represent_ordereddict(dumper, data):
    value = []

    for item_key, item_value in data.items():
        node_key = dumper.represent_data(item_key)
        node_value = dumper.represent_data(item_value)

        value.append((node_key, node_value))

    return yaml.nodes.MappingNode(u'tag:yaml.org,2002:map', value)

yaml.add_representer(OrderedDict, represent_ordereddict)

def represent_int(dumper, data):
    return dumper.represent_int(hex(data))

yaml.add_representer(int, represent_int)

def splat_names(base, parts):
    names = []
    for part in parts:
        if part.startswith("STM32"):
            names.append( base )
        else:
            names.append( base[0: len(base) - len(part)] + part)

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
            splatted = splat_names(current_base, parts )
            current_base = splatted[0]
            cleaned = cleaned + splatted
        elif name.startswith("STM32"):
            current_base = name
            cleaned.append(name)
        else:
            cleaned.append( current_base[0: len(current_base) - len(name)] + name)
    return cleaned

memories = []

def parse_files(dir):
    for f in sorted(glob(dir + '/*.xml')):
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
                    configs = [ configs ]
                ram_addr = int(configs[0]['Parameters']['@address'], 16)
                ram_size = int(configs[0]['Parameters']['@size'], 16)
                #print( f'ram {addr} {size}')
            if peripheral['Name'] == 'Embedded Flash' and flash_size is None:
                configs = peripheral['Configuration']
                if type(configs) != list:
                    configs = [ configs ]
                flash_addr = int(configs[0]['Parameters']['@address'], 16)
                flash_size = int(configs[0]['Parameters']['@size'], 16)
                #print( f'flash {addr} {size}')

        chunk = OrderedDict( {
            'device-id': int(device_id, 16),
            'names': names,
        })

        if ram_size is not None:
            chunk['ram'] = OrderedDict( {
                'address': ram_addr,
                'bytes': ram_size,
            })

        if flash_size is not None:
            chunk['flash'] = OrderedDict( {
                'address': flash_addr,
                'bytes': flash_size,
            })

        memories.append( chunk )

dir = sys.argv[1]

parse_files(dir)

with open('data/memories.yaml', 'w') as f:
    f.write(yaml.dump(memories, width=500))

#print(yaml.dump(memories, width=500))