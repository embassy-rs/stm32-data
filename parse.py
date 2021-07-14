#!/usr/bin/env python3

import xmltodict
import yaml
try:
    from yaml import CSafeLoader as SafeLoader
except ImportError:
    from yaml import SafeLoader

import re
import json
import os
from collections import OrderedDict
from glob import glob


def removeprefix(value: str, prefix: str, /) -> str:
    if value.startswith(prefix):
        return value[len(prefix):]
    else:
        return value[:]


def corename(d):
    if m := re.match('.*Cortex-M(\d+)(\+?)', d):
        name = "cm" + str(m.group(1))
        if m.group(2) == "+":
            name += "p"
        return name


def removesuffix(value: str, suffix: str, /) -> str:
    if value.endswith(suffix):
        return value[:-len(suffix)]
    else:
        return value[:]


def represent_ordereddict(dumper, data):
    value = []

    for item_key, item_value in data.items():
        node_key = dumper.represent_data(item_key)
        node_value = dumper.represent_data(item_value)

        value.append((node_key, node_value))

    return yaml.nodes.MappingNode(u'tag:yaml.org,2002:map', value)


yaml.add_representer(OrderedDict, represent_ordereddict)


def hexint_presenter(dumper, data):
    if data > 0x10000:
        return dumper.represent_int(hex(data))
    else:
        return dumper.represent_int(data)


yaml.add_representer(int, hexint_presenter)


def children(x, key):
    r = x.get(key)
    if r is None:
        return []
    if type(r) is list:
        return r
    return [r]


headers_parsed = {}
header_map = {}
with open('header_map.yaml', 'r') as f:
    y = yaml.load(f, Loader=SafeLoader)
    for header, chips in y.items():
        for chip in chips.split(','):
            header_map[chip.strip().lower()] = header.lower()


def find_header(model):
    # for a, b in header_map:
    #    model = re.sub(a, b, model, flags=re.IGNORECASE)
    model = model.lower()

    # if it's in the map, just go
    if r := header_map.get(model):
        return r

    # if not, find it by regex, taking `x` meaning `anything`
    res = []
    for h in headers_parsed.keys():
        if re.match('^' + h.replace('x', '.') + '$', model):
            res.append(h)

    if len(res) == 0:
        return None
    assert len(res) == 1
    return res[0]


def paren_ok(val):
    n = 0
    for c in val:
        if c == '(':
            n += 1
        if c == ')':
            n -= 1
        if n < 0:
            return False
    return n == 0

# warning: horrible abomination ahead


def parse_value(val, defines):
    val = val.strip()
    if val == '':
        return 0
    if m := re.match('((0x[0-9a-fA-F]+|\\d+))(|u|ul|U|UL)$', val):
        return int(m.group(1), 0)
    if m := re.match('([0-9A-Za-z_]+)$', val):
        return defines.get(m.group(1), 0)
    if m := re.match('\\((.*)\\)$', val):
        if paren_ok(m.group(1)):
            return parse_value(m.group(1), defines)
    if m := re.match('\\*?\\([0-9A-Za-z_]+ *\\*?\\)(.*)$', val):
        return parse_value(m.group(1), defines)
    # if m := re.match('\\*?\\(u?int(8|16|32|64)_t\\ *)(.*)$', val):
    #    return parse_value(m.group(1), defines)
    if m := re.match('(.*)<<(.*)$', val):
        return (parse_value(m.group(1), defines) << parse_value(m.group(2), defines)) & 0xFFFFFFFF
    if m := re.match('(.*)>>(.*)$', val):
        return parse_value(m.group(1), defines) >> parse_value(m.group(2), defines)
    if m := re.match('(.*)\\|(.*)$', val):
        return parse_value(m.group(1), defines) | parse_value(m.group(2), defines)
    if m := re.match('(.*)&(.*)$', val):
        return parse_value(m.group(1), defines) | parse_value(m.group(2), defines)
    if m := re.match('~(.*)$', val):
        return (~parse_value(m.group(1), defines)) & 0xFFFFFFFF
    if m := re.match('(.*)\\+(.*)$', val):
        return parse_value(m.group(1), defines) + parse_value(m.group(2), defines)
    if m := re.match('(.*)-(.*)$', val):
        return parse_value(m.group(1), defines) - parse_value(m.group(2), defines)
    raise Exception("can't parse: " + val)


def parse_header(f):
    irqs = {}
    defines = {}
    cores = []
    cur_core = 'all'

    accum = ''
    for l in open(f, 'r', encoding='utf-8', errors='ignore'):
        l = l.strip()
        l = accum + l
        if l.endswith('\\'):
            accum = l[:-1]
            continue
        accum = ''

        # Scoped by a single core
        if m := re.match('.*if defined.*CORE_CM(\\d+)(PLUS)?.*', l):
            cur_core = "cm" + str(m.group(1))
            if m.group(2) != None:
                cur_core += "p"
            #print("Cur core is ", cur_core, "matched", l)
            found = False
            for core in cores:
                if core == cur_core:
                    found = True
            if not found:
                cores.append(cur_core)
            #print("Switching to core", cur_core, "for", f)
        elif m := re.match('.*else.*', l):
            cur_core = "all"
            if m := re.match('.*else.*CORE_CM(\\d+)(PLUS)?.*', l):
                cur_core = "cm" + str(m.group(1))
                if m.group(2) != None:
                    cur_core += "p"
                #print("Cur core is ", cur_core, "matched", l)
            elif len(cores) > 1:
                # Pick the second core assuming we've already parsed one
                cur_core = cores[1]

            found = False
            for core in cores:
                if core == cur_core:
                    found = True
            if not found:
                cores.append(cur_core)
            #print("Switching to core", cur_core, "for", f)
        elif m := re.match('.*endif.*', l):
            #print("Switching to common core for", f)
            cur_core = "all"

        if cur_core not in irqs:
            #print("Registering new core", cur_core)
            irqs[cur_core] = {}
        if cur_core not in defines:
            defines[cur_core] = {}

        if m := re.match('([a-zA-Z0-9_]+)_IRQn += (\\d+),? +/\\*!< (.*) \\*/', l):
            #print("Found irq for", cur_core)
            irqs[cur_core][m.group(1)] = int(m.group(2))

        if m := re.match('#define +([0-9A-Za-z_]+)\\(', l):
            defines[cur_core][m.group(1)] = -1
        if m := re.match('#define +([0-9A-Za-z_]+) +(.*)', l):
            name = m.group(1)
            val = m.group(2)
            name = name.strip()
            if name == 'FLASH_SIZE':
                continue
            val = val.split('/*')[0].strip()
            val = parse_value(val, defines[cur_core])
            #print("Found define for", cur_core)
            defines[cur_core][name] = val

    #print("Found", len(cores), "cores for", f)
    #print("Found", len(irqs['all']), "shared interrupts for", f)

    if len(cores) == 0:
        cores.append("all")

    for core in cores:
        if core != "all":
            irqs[core].update(irqs['all'])
            defines[core].update(defines['all'])

    return {
        'cores': cores,
        'interrupts': irqs,
        'defines': defines,
    }


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

    # I2S is just SPI on disguise
    'I2S1',
    'I2S2',
    'I2S3',
    'I2S4',
    'I2S5',
    'I2S6',
    'I2S7',
    'I2S8',

    # These are software libraries
    'FREERTOS',
    'PDM2PCM',
    'FATFS',
    'CRC',
    'LIBJPEG',
    'MBEDTLS',
    'LWIP',
    'USB_HOST',
    'USB_DEVICE',
    'GUI_INTERFACE',
    'TRACER_EMB',
]

perimap = [
    ('UART:sci2_v1_1', 'usart_v1/UART'),
    ('UART:sci2_v1_2', 'usart_v1/UART'),
    ('UART:sci2_v1_2_F1', 'usart_v1/UART'),
    ('UART:sci2_v2_1', 'usart_v2/UART'),
    # ('UART:sci2_v3_0', 'usart_v3/UART'),
    # ('UART:sci2_v3_1', 'usart_v3/UART'),

    ('.*:USART:sci2_v1_1', 'usart_v1/USART'),
    ('.*:USART:sci2_v1_2_F1', 'usart_v1/USART'),
    ('.*:USART:sci2_v1_2', 'usart_v1/USART'),
    ('.*:USART:sci2_v2_0', 'usart_v2/USART'),
    ('.*:USART:sci2_v2_1', 'usart_v2/USART'),
    ('.*:USART:sci2_v2_2', 'usart_v2/USART'),
    ('.*:USART:sci3_v1_0', 'usart_v2/USART'),
    ('.*:USART:sci3_v1_1', 'usart_v2/USART'),
    ('.*:USART:sci3_v1_2', 'usart_v3/USART'),
    ('.*:USART:sci3_v2_0', 'usart_v3/USART'),
    ('.*:USART:sci3_v2_1', 'usart_v3/USART'),
    ('.*:UART:sci2_v3_0', 'usart_v3/USART'),
    ('.*:RNG:rng1_v1_1', 'rng_v1/RNG'),
    ('.*:RNG:rng1_v2_0', 'rng_v1/RNG'),
    ('.*:RNG:rng1_v2_1', 'rng_v1/RNG'),
    ('.*:RNG:rng1_v3_1', 'rng_v1/RNG'),
    ('.*:SPI:spi2s1_v2_2', 'spi_v1/SPI'),
    ('.*:SPI:spi2s1_v3_3', 'spi_v2/SPI'),
    ('.*:SPI:spi2s1_v3_1', 'spi_v2/SPI'),
    ('.*:SPI:spi2s2_v1_1', 'spi_v3/SPI'),
    ('.*:SPI:spi2s2_v1_0', 'spi_v3/SPI'),
    ('.*:I2C:i2c1_v1_5', 'i2c_v1/I2C'),
    ('.*:I2C:i2c2_v1_1', 'i2c_v2/I2C'),
    ('.*:I2C:i2c2_v1_1F7', 'i2c_v2/I2C'),
    ('.*:DAC:dacif_v2_0', 'dac_v2/DAC'),
    ('.*:DAC:dacif_v3_0', 'dac_v2/DAC'),
    ('.*:ADC:aditf5_v2_0', 'adc_v3/ADC'),
    ('.*:ADC_COMMON:aditf5_v2_0', 'adccommon_v3/ADC_COMMON'),
    ('.*:ADC_COMMON:aditf4_v3_0_WL', 'adccommon_v3/ADC_COMMON'),
    ('STM32F0.*:SYS:.*', 'syscfg_f0/SYSCFG'),
    ('STM32F4.*:SYS:.*', 'syscfg_f4/SYSCFG'),
    ('STM32L4.*:SYS:.*', 'syscfg_l4/SYSCFG'),
    ('STM32L0.*:SYS:.*', 'syscfg_l0/SYSCFG'),
    ('STM32H7.*:SYS:.*', 'syscfg_h7/SYSCFG'),
    ('STM32WB55.*:SYS:.*', 'syscfg_wb55/SYSCFG'),
    ('STM32WL.*:SYS:.*', 'syscfg_wl5x/SYSCFG'),
    ('STM32L0.*:RCC:.*', 'rcc_l0/RCC'),
    ('STM32L4.*:RCC:.*', 'rcc_l4/RCC'),
    ('STM32F4.*:RCC:.*', 'rcc_f4/RCC'),
    ('STM32WL.*:RCC:.*', 'rcc_wl5x/RCC'),
    ('STM32F0.0.*:RCC:.*', 'rcc_f0x0/RCC'),
    ('STM32F0.*:RCC:.*', 'rcc_f0/RCC'),
    ('.*:STM32H7AB_rcc_v1_0', ''),  # rcc_h7ab/RCC
    ('.*:STM32H7_rcc_v1_0', 'rcc_h7/RCC'),
    ('.*:STM32W_rcc_v1_0', 'rcc_wb55/RCC'),
    ('.*:STM32L0_crs_v1_0', 'crs_l0/CRS'),
    ('.*SDMMC:sdmmc2_v1_0', 'sdmmc_v2/SDMMC'),
    ('.*:STM32H7_pwr_v1_0', 'pwr_h7/PWR'),
    ('.*:STM32H7_flash_v1_0', 'flash_h7/FLASH'),
    ('.*:STM32F0_flash_v1_0', 'flash_f0/FLASH'),
    ('.*TIM\d.*:gptimer.*', 'timer_v1/TIM_GP16'),
    ('.*ETH:ethermac110_v3_0', 'eth_v2/ETH'),

    ('.*:STM32F0_dbgmcu_v1_0', 'dbgmcu_f0/DBGMCU'),
    ('.*:STM32F1_dbgmcu_v1_0', 'dbgmcu_f1/DBGMCU'),
    ('.*:STM32F2_dbgmcu_v1_0', 'dbgmcu_f2/DBGMCU'),
    ('.*:STM32F3_dbgmcu_v1_0', 'dbgmcu_f3/DBGMCU'),
    ('.*:STM32F4_dbgmcu_v1_0', 'dbgmcu_f4/DBGMCU'),
    ('.*:STM32F7_dbgmcu_v1_0', 'dbgmcu_f7/DBGMCU'),
    ('.*:STM32G0_dbgmcu_v1_0', 'dbgmcu_g0/DBGMCU'),
    ('.*:STM32G4_dbgmcu_v1_0', 'dbgmcu_g4/DBGMCU'),
    ('.*:STM32H7_dbgmcu_v1_0', 'dbgmcu_h7/DBGMCU'),
    ('.*:STM32L0_dbgmcu_v1_0', 'dbgmcu_l0/DBGMCU'),
    ('.*:STM32L4_dbgmcu_v1_0', 'dbgmcu_l4/DBGMCU'),
    ('.*:STM32WB_dbgmcu_v1_0', 'dbgmcu_wb/DBGMCU'),
    ('.*:STM32WL_dbgmcu_v1_0', 'dbgmcu_wl/DBGMCU'),

    ('.*:IPCC:v1_0', 'ipcc_v1/IPCC'),
    ('.*:DMAMUX:v1', 'dmamux_v1/DMAMUX'),

    ('.*:BDMA:DMA', 'bdma_v1/DMA'),
    ('STM32L4[PQRS].*:.*:DMA', 'bdma_v1/DMA'),  # L4+
    ('STM32L[04].*:.*:DMA', 'bdma_v2/DMA'),  # L0, L4 non-plus (since plus is handled above)
    ('STM32F[247].*:.*:DMA', 'dma_v2/DMA'),
    ('STM32H7.*:.*:DMA', 'dma_v1/DMA'),
    ('.*:DMA', 'bdma_v1/DMA'),
]

def match_peri(peri):
    for r, block in perimap:
        if re.match('^'+r+'$', peri):
            if block == '':
                return None
            return block
    return None


def find_af(gpio_af, peri_name, pin_name, signal_name):
    if gpio_af in af:
        if pin_name in af[gpio_af]:
            if peri_name + '_' + signal_name in af[gpio_af][pin_name]:
                return af[gpio_af][pin_name][peri_name + '_' + signal_name]
    return None


all_mcu_files = {}
per_mcu_files = {}


def parse_documentations():
    print("linking files and documents")
    with open('sources/mcufinder/files.json', 'r') as j:
        files = json.load(j)
        for file in files['Files']:
            file_id = file['id_file']
            if file_id not in all_mcu_files:
                all_mcu_files[file_id] = OrderedDict({
                    'name': file['name'],
                    'title': file['title'],
                    'url': file['URL'],
                    'type': file['type'],
                })

    with open('sources/mcufinder/mcus.json', 'r') as j:
        mcus = json.load(j)
        for mcu in mcus['MCUs']:
            rpn = mcu['RPN']
            if rpn not in per_mcu_files:
                per_mcu_files[rpn] = []
            for file in mcu['files']:
                per_mcu_files[rpn].append(file['file_id'])


def documents_for(chip_name, type):
    docs = []
    for id in per_mcu_files[chip_name]:
        if id in all_mcu_files:
            file = all_mcu_files[id]
            if file['type'] == type:
                docs.append(OrderedDict({
                    'title': file['title'],
                    'name': file['name'],
                    'url': file['url'],
                }))

    return docs


def parse_headers():
    os.makedirs('sources/headers_parsed', exist_ok=True)
    print('loading headers...')
    for f in glob('sources/headers/*.h'):
        # if 'stm32f4' not in f: continue
        ff = removeprefix(f, 'sources/headers/')
        ff = removesuffix(ff, '.h')

        try:
            with open('sources/headers_parsed/{}.json'.format(ff), 'r') as j:
                res = json.load(j)
        except:
            print(f)
            res = parse_header(f)
            with open('sources/headers_parsed/{}.json'.format(ff), 'w') as j:
                json.dump(res, j)

        headers_parsed[ff] = res


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
        ('(STM32......).x', '\\1'),
    ]

    for a, b in name_map:
        r, n = re.subn('^'+a+'$', b, x)
        if n != 0:
            return r
    raise Exception("bad name: {}".format(x))


def parse_chips():
    os.makedirs('data/chips', exist_ok=True)

    chips = {}

    for f in sorted(glob('sources/cubedb/mcu/STM32*.xml')):
        if 'STM32MP' in f:
            continue
        print(f)

        r = xmltodict.parse(open(f, 'rb'))['Mcu']

        package_names = expand_name(r['@RefName'])
        package_rams = r['Ram']
        package_flashs = r['Flash']
        if type(package_rams) != list:
            package_rams = [package_rams]*len(package_names)
        if type(package_flashs) != list:
            package_flashs = [package_flashs]*len(package_names)
        for package_i, package_name in enumerate(package_names):
            chip_name = chip_name_from_package_name(package_name)
            flash = int(package_flashs[package_i])
            ram = int(package_rams[package_i])
            gpio_af = next(filter(lambda x: x['@Name'] == 'GPIO', r['IP']))['@Version']
            gpio_af = removesuffix(gpio_af, '_gpio_v1_0')

            dma = next(filter(lambda x: x['@Name'] == 'DMA', r['IP']), None)
            bdma = next(filter(lambda x: x['@Name'] == 'BDMA', r['IP']), None)

            if dma is not None:
                dma = dma['@Version']
            if bdma is not None:
                bdma = bdma['@Version']
            #dma = next(filter(lambda x: x['@Name'] == 'DMA', r['IP']), None)['@Version']
            #bdma = next(filter(lambda x: x['@Name'] == 'BDMA', r['IP']), None)['@Version']

            rcc = next(filter(lambda x: x['@Name'] == 'RCC', r['IP']))['@Version']

            rcc = removesuffix(rcc, '-rcc_v1_0')
            rcc = removesuffix(rcc, '_rcc_v1_0')

            core = r['Core']
            family = r['@Family']

            cores = []
            if isinstance(core, list):
                for core in core:
                    cores.append(OrderedDict(
                        {
                            'name': corename(core),
                            'peripherals': {},
                        }))
            else:
                cores.append(OrderedDict(
                    {
                        'name': corename(core),
                        'peripherals': {},
                    }))

            if chip_name not in chips:
                chips[chip_name] = OrderedDict({
                    'name': chip_name,
                    'family': family,
                    'line': r['@Line'],
                    'packages': [],
                    'datasheet': None,
                    'reference-manual': None,
                    'flash': flash,
                    'ram': ram,
                    'cores': cores,
                    'peripherals': {},
                    'pins': {},
                    'application-notes': [],
                    'rcc': rcc,  # temporarily stashing it here
                    'dma': dma,  # temporarily stashing it here
                    'bdma': bdma,  # temporarily stashing it here
                })

            chips[chip_name]['packages'].append(OrderedDict({
                'name': package_name,
                'package': r['@Package'],
            }))

            if chip_name in per_mcu_files:
                if len(ds := documents_for(chip_name, 'Datasheet')) >= 1:
                    chips[chip_name]['datasheet'] = ds[0]
                if len(rm := documents_for(chip_name, 'Reference manual')) >= 1:
                    chips[chip_name]['reference-manual'] = rm[0]
                chips[chip_name]['application-notes'] = documents_for(chip_name, 'Application note')

            # Some packages have some peripehrals removed because the package had to
            # remove GPIOs useful for that peripheral. So we merge all peripherals from all packages.
            peris = chips[chip_name]['peripherals']
            pins = chips[chip_name]['pins']

            for ip in r['IP']:
                pname = ip['@InstanceName']
                pkind = ip['@Name']+':'+ip['@Version']
                pkind = removesuffix(pkind, '_Cube')

                if pname == 'SYS':
                    pname = 'SYSCFG'
                if pname in FAKE_PERIPHERALS:
                    continue
                if pname.startswith('ADC'):
                    if not 'ADC_COMMON' in peris:
                        peris['ADC_COMMON'] = 'ADC_COMMON:'+removesuffix(ip['@Version'], '_Cube')
                peris[pname] = pkind
                pins[pname] = []

            for pin in r['Pin']:
                pin_name = pin['@Name']
                pin_name = pin_name.split(' ', 1)[0]
                pin_name = pin_name.split('-', 1)[0]
                if 'Signal' in pin:
                    signals = []
                    if not type(pin['Signal']) is list:
                        signals.append(pin['Signal'])
                    else:
                        signals = pin['Signal']

                    for signal in signals:
                        signal_name = signal['@Name']
                        parts = signal_name.split('_', 1)
                        if len(parts) == 1:
                            continue
                        peri_name = parts[0]
                        signal_name = parts[1]
                        if signal_name.startswith("EXTI"):
                            continue
                        if not peri_name in pins:
                            pins[peri_name] = []
                        entry = OrderedDict({
                            'pin': pin_name,
                            'signal': signal_name,
                        })
                        af_num = find_af(gpio_af, peri_name, pin_name, signal_name)
                        if af_num is not None:
                            entry['af'] = af_num

                        pins[peri_name].append(entry)

    for chip_name, chip in chips.items():
        print(f'* processing chip {chip_name}')
        rcc = chip['rcc']
        del chip['rcc']

        chip_dma = chip['dma']
        del chip['dma']

        chip_bdma = chip['bdma']
        del chip['bdma']

        h = find_header(chip_name)
        if h is None:
            raise Exception("missing header for {}".format(chip_name))
        h = headers_parsed[h]

        # print("Got", len(chip['cores']), "cores")
        for core in chip['cores']:
            core_name = core['name']
            if not core_name in h['interrupts'] or not core_name in h['defines']:
                core_name = 'all'
            #print("Defining for core", core_name)

            # Gather all interrupts and defines for this core

            interrupts = h['interrupts'][core_name]
            defines = h['defines'][core_name]

            core['interrupts'] = interrupts
            core['dma_channels'] = {}
            if chip_dma in dma_channels:
                core['dma_channels'].update(dma_channels[chip_dma]['channels'])
            if chip_bdma in dma_channels:
                core['dma_channels'].update(dma_channels[chip_bdma]['channels'])
    #        print("INterrupts for", core, ":", interrupts)
            #print("Defines for", core, ":", defines)

            peris = {}
            for pname, pkind in chip['peripherals'].items():
                addr = defines.get(pname)
                if addr is None:
                    if pname == 'ADC_COMMON':
                            addr = defines.get('ADC1_COMMON')
                            if addr is None:
                                addr = defines.get('ADC12_COMMON')
                                if addr is None:
                                    addr = defines.get('ADC123_COMMON')
                if addr is None:
                    continue

                p = OrderedDict({
                    'address': addr,
                    'kind': pkind,
                })

                if pname in clocks[rcc]:
                    p['clock'] = clocks[rcc][pname]

                if block := match_peri(chip_name+':'+pname+':'+pkind):
                    p['block'] = block

                if pname in chip['pins']:
                    if len(chip['pins'][pname]) > 0:
                        p['pins'] = chip['pins'][pname]

                if pname in dma_channels[chip_dma]['peripherals']:
                    if 'channels' in dma_channels[chip_dma]['peripherals'][pname]:
                        p['dma_channels'] = dma_channels[chip_dma]['peripherals'][pname]['channels']
                    if 'requests' in dma_channels[chip_dma]['peripherals'][pname]:
                        p['dma_requests'] = dma_channels[chip_dma]['peripherals'][pname]['requests']

                peris[pname] = p

            family_extra = "data/extra/family/" + chip['family'] + ".yaml"
            if os.path.exists(family_extra):
                with open(family_extra) as extra_f:
                    extra = yaml.load(extra_f, Loader=SafeLoader)
                    for (extra_name, extra_p) in extra['peripherals'].items():
                        peris[extra_name] = extra_p

            # Handle GPIO specially.
            for p in range(20):
                port = 'GPIO' + chr(ord('A')+p)
                if addr := defines.get(port + '_BASE'):
                    block = 'gpio_v2/GPIO'
                    if chip['family'] == 'STM32F1':
                        block = 'gpio_v1/GPIO'

                    p = OrderedDict({
                        'address': addr,
                        'block': block,
                    })
                    peris[port] = p

            # Handle DMA specially.
            for dma in ('DMA1', 'DMA2', 'BDMA'):
                if addr := defines.get(dma + '_BASE'):
                    p = OrderedDict({
                        'address': addr,
                    })
                    if block := match_peri(chip_name+':'+dma+':DMA'):
                        p['block'] = block

                    peris[dma] = p

            # DMAMUX is not in the cubedb XMLs
            for dma in ('DMAMUX', 'DMAMUX1', "DMAMUX2"):
                if addr := defines.get(dma+'_BASE'):
                    kind = 'DMAMUX:v1'
                    dbg_peri = OrderedDict({
                        'address': addr,
                        'kind': kind,
                    })
                    if block := match_peri(chip_name+':'+dma+':'+kind):
                        dbg_peri['block'] = block
                    peris[dma] = dbg_peri


            # EXTI is not in the cubedb XMLs
            if addr := defines.get('EXTI_BASE'):
                if chip_name.startswith("STM32WB55"):
                    block = 'exti_wb55/EXTI'
                else:
                    block = 'exti_v1/EXTI'

                peris['EXTI'] = OrderedDict({
                    'address': addr,
                    'kind': 'EXTI',
                    'block': block,
                })

            # FLASH is not in the cubedb XMLs
            if addr := defines.get('FLASH_R_BASE'):
                kind = 'FLASH:' + chip_name[:7] + '_flash_v1_0'
                flash_peri = OrderedDict({
                    'address': addr,
                    'kind': kind,
                })
                if block := match_peri(kind):
                    flash_peri['block'] = block
                peris['FLASH'] = flash_peri

            # DBGMCU is not in the cubedb XMLs
            if addr := defines.get('DBGMCU_BASE') or defines.get('DBG_BASE'):
                kind = 'DBGMCU:' + chip_name[:7] + '_dbgmcu_v1_0'
                dbg_peri = OrderedDict({
                    'address': addr,
                    'kind': kind,
                })
                if block := match_peri(kind):
                    dbg_peri['block'] = block
                peris['DBGMCU'] = dbg_peri

            # CRS is not in the cubedb XMLs
            if addr := defines.get('CRS_BASE'):
                kind = 'CRS:' + chip_name[:7] + '_crs_v1_0'
                crs_peri = OrderedDict({
                    'address': addr,
                    'kind': kind,
                })
                if block := match_peri(kind):
                    crs_peri['block'] = block
                peris['CRS'] = crs_peri

            core['peripherals'] = peris

            if 'block' in core['peripherals']['RCC']:
                rcc_block = core['peripherals']['RCC']['block']

                for (name, body) in core['peripherals'].items():
                    if 'clock' not in body:
                        if (peri_clock := match_peri_clock(rcc_block, name)) is not None:
                            core['peripherals'][name]['clock'] = peri_clock

        # remove all pins from the root of the chip before emitting.
        del chip['pins']
        del chip['peripherals']

        with open('data/chips/'+chip_name+'.yaml', 'w') as f:
            f.write(yaml.dump(chip, width=500))


af = {}


def parse_gpio_af():
    #os.makedirs('data/gpio_af', exist_ok=True)
    for f in glob('sources/cubedb/mcu/IP/GPIO-*_gpio_v1_0_Modes.xml'):
        if 'STM32F1' in f:
            continue

        ff = removeprefix(f, 'sources/cubedb/mcu/IP/GPIO-')
        ff = removesuffix(ff, '_gpio_v1_0_Modes.xml')

        pins = {}

        r = xmltodict.parse(open(f, 'rb'))
        for pin in r['IP']['GPIO_Pin']:
            pin_name = pin['@Name']

            # Blacklist non-pins
            if pin_name == 'PDR_ON':
                continue

            # Cleanup pin name
            pin_name = pin_name.split('/')[0]
            pin_name = pin_name.split('-')[0]
            pin_name = pin_name.split(' ')[0]
            pin_name = pin_name.split('_')[0]
            pin_name = pin_name.split('(')[0]
            pin_name = removesuffix(pin_name, 'OSC32')
            pin_name = removesuffix(pin_name, 'BOOT0')

            # Extract AFs
            afs = {}
            for signal in children(pin, 'PinSignal'):
                func = signal['@Name']
                afn = signal['SpecificParameter']['PossibleValue'].split('_')[1]
                afn = int(removeprefix(afn, 'AF'))
                afs[func] = afn

            pins[pin_name] = afs

        # with open('data/gpio_af/'+ff+'.yaml', 'w') as f:
            # f.write(yaml.dump(pins))

        af[ff] = pins


dma_channels = {}


def parse_dma():
    for f in glob('sources/cubedb/mcu/IP/*DMA-*Modes.xml'):
        ff = removeprefix(f, 'sources/cubedb/mcu/IP/')
        if not ( ff.startswith('B') or ff.startswith( 'D' ) ):
            continue
        ff = removeprefix(ff, 'DMA-')
        ff = removeprefix(ff, 'BDMA-')
        ff = removesuffix(ff, '_Modes.xml')

        r = xmltodict.parse(open(f, 'rb'),  force_list={'Mode', 'RefMode'})

        chip_dma = {
            'channels': {},
            'peripherals': {},
        }

        for dma in r['IP']['ModeLogicOperator']['Mode']:
            dma_peri_name = dma['@Name']
            if ' Context' in dma_peri_name:
                continue
            channels = dma['ModeLogicOperator']['Mode']
            if len(channels) == 1:
                requests = next(filter(lambda x: x['@Name'] == 'Request', r['IP']['RefParameter']))
                request_num = 0
                for request in requests['PossibleValue']:
                    target_name = request['@Comment']
                    parts = target_name.split('_')
                    target_peri_name = parts[0]
                    if len(parts) < 2:
                        event = target_peri_name
                    else:
                        event = target_name.split('_')[1]
                    if target_name != 'MEMTOMEM':
                        if target_peri_name not in chip_dma['peripherals']:
                            chip_dma['peripherals'][target_peri_name] = {}
                        peri_dma = chip_dma['peripherals'][target_peri_name]
                        if 'requests' not in peri_dma:
                            peri_dma['requests'] = {}
                        if event not in peri_dma['requests']:
                            peri_dma['requests'][event] = request_num
                        #event_dma = peri_dma['requests'][event]
                        # event_dma.append( OrderedDict( {
                            # 'channel': 'DMAMUX',
                            # 'request': request_num
                        # } ))
                    request_num += 1
                for n in dma_peri_name.split(","):
                    n = n.strip()
                    if result := re.match('.*' + n + '_(Channel|Stream)\[(\d+)-(\d+)\]', channels[0]['@Name']):
                        low = int(result.group(2))
                        high = int(result.group(3))
                        for i in range(low, high+1):
                            chip_dma['channels'][n+'_'+str(i)] = OrderedDict({
                                'dma': n,
                                'channel': i,
                            })

            else:
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
                    chip_dma['channels'][dma_peri_name + '_' + channel_name] = OrderedDict({
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
                            target_events = [target_peri_name]
                        else:
                            target_events = target_name.split('_')[1].split('/')
                        if target_name != 'MEMTOMEM':
                            if target_peri_name not in chip_dma['peripherals']:
                                chip_dma['peripherals'][target_peri_name] = {}
                            peri_dma = chip_dma['peripherals'][target_peri_name]
                            for event in target_events:
                                if ':' in event:
                                    event = event.split(':')[0]
                                if 'channels' not in peri_dma:
                                    peri_dma['channels'] = {}
                                if event not in peri_dma['channels']:
                                    peri_dma['channels'][event] = []
                                event_dma = peri_dma['channels'][event]
                                entry = OrderedDict({
                                    'channel': dma_peri_name + '_' + channel_name,
                                })
                                if original_target_name in requests:
                                    entry['request'] = requests[original_target_name]
                                event_dma.append(entry)

                # Make sure all channels numbers start at 0
                if min(map(int, channel_names)) != 0:
                    for name in channel_names:
                        chip_dma['channels'][dma_peri_name + '_' + name]['channel'] -= 1

        dma_channels[ff] = chip_dma


clocks = {}

def parse_clocks():
    for f in glob('sources/cubedb/mcu/IP/RCC-*rcc_v1_0_Modes.xml'):
        ff = removeprefix(f, 'sources/cubedb/mcu/IP/RCC-')
        ff = removesuffix(ff, '_rcc_v1_0_Modes.xml')
        ff = removesuffix(ff, '-rcc_v1_0_Modes.xml')
        chip_clocks = {}
        r = xmltodict.parse(open(f, 'rb'))
        for ref in r['IP']['RefParameter']:
            name = ref['@Name']
            if name.startswith("APB") and name.endswith("Freq_Value") and not name.endswith("TimFreq_Value") and '@IP' in ref:
                name = removesuffix(name, "Freq_Value")
                peripherals = ref['@IP']
                peripherals = peripherals.split(",")
                for p in peripherals:
                    chip_clocks[p] = name

        clocks[ff] = chip_clocks

peripheral_to_clock = {}

def parse_rcc_regs():
    print("parsing RCC registers")
    for f in glob('data/registers/rcc_*'):
        ff = removeprefix(f, 'data/registers/rcc_')
        ff = removesuffix(ff, '.yaml')
        family_clocks = {}
        with open(f, 'r') as yaml_file:
            y = yaml.load(yaml_file, Loader=SafeLoader)
            for (key, body) in y.items():
                if key.startswith("fieldset/A") and key.endswith("ENR"):
                    clock = removesuffix(key, "ENR")
                    clock = removeprefix(clock, "fieldset/")
                    clock = removesuffix(clock, "L")
                    clock = removesuffix(clock, "H")
                    for field in body['fields']:
                        if field['name'].endswith('EN'):
                            peri = removesuffix(field['name'], 'EN')
                            family_clocks[peri] = clock
        peripheral_to_clock['rcc_' + ff + '/RCC'] = family_clocks

def match_peri_clock(rcc_block, peri_name):
    if rcc_block in peripheral_to_clock:
        family_clocks = peripheral_to_clock[rcc_block]
        if peri_name in family_clocks:
            return family_clocks[peri_name]
        #print("found no clock for ", peri_name)
        if peri_name.endswith("1"):
            return match_peri_clock(rcc_block, removesuffix(peri_name, "1"))
        return None

parse_rcc_regs()
parse_documentations()
parse_dma()
parse_gpio_af()
parse_headers()
parse_clocks()
parse_chips()
