import xmltodict
import yaml
import re
import json
import os
from collections import OrderedDict
from glob import glob

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
    y = yaml.load(f, Loader=yaml.SafeLoader)
    for header, chips in y.items():
        for chip in chips.split(','):
            header_map[chip.strip().lower()] = header.lower()

def find_header(model):
    #for a, b in header_map:
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
        if c == '(': n += 1
        if c == ')': n -= 1
        if n < 0: return False
    return n == 0

# warning: horrible abomination ahead
def parse_value(val, defines):
    val = val.strip()
    if val == '': return 0
    if m := re.match('((0x[0-9a-fA-F]+|\\d+))(|u|ul|U|UL)$', val):
        return int(m.group(1), 0)
    if m := re.match('([0-9A-Za-z_]+)$', val):
        return defines.get(m.group(1), 0)
    if m := re.match('\\((.*)\\)$', val):
        if paren_ok(m.group(1)):
            return parse_value(m.group(1), defines)
    if m := re.match('\\*?\\([0-9A-Za-z_]+ *\\*?\\)(.*)$', val):
        return parse_value(m.group(1), defines)
    #if m := re.match('\\*?\\(u?int(8|16|32|64)_t\\ *)(.*)$', val):
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

    accum = ''
    for l in open(f, 'r', encoding='utf-8', errors='ignore'):
        l = l.strip()
        l = accum + l
        if l.endswith('\\'):
            accum = l[:-1]
            continue
        accum = ''

        if m := re.match('([a-zA-Z0-9_]+)_IRQn += (\\d+),? +/\\*!< (.*) \\*/', l):
            irqs[m.group(1)] = int(m.group(2))

        if m := re.match('#define +([0-9A-Za-z_]+)\\(', l):
            defines[m.group(1)] = -1
        if m := re.match('#define +([0-9A-Za-z_]+) +(.*)', l):
            name = m.group(1)
            val = m.group(2)
            name = name.strip()
            if name == 'FLASH_SIZE': continue
            val = val.split('/*')[0].strip()
            val = parse_value(val, defines)
            defines[name] = val

    return {
        'interrupts': irqs,
        'defines': defines,
    }


def expand_name(name):
    if '(' not in name: return [name]
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
    #('UART:sci2_v3_0', 'usart_v3/UART'),
    #('UART:sci2_v3_1', 'usart_v3/UART'),

    ('.*:USART:sci2_v1_1', 'usart_v1/USART'),
    ('.*:USART:sci2_v1_2_F1', 'usart_v1/USART'),
    ('.*:USART:sci2_v1_2', 'usart_v1/USART'),
    ('.*:USART:sci2_v2_0', 'usart_v2/USART'),
    ('.*:USART:sci2_v2_1', 'usart_v2/USART'),
    ('.*:USART:sci2_v2_2', 'usart_v2/USART'),
    ('.*:USART:sci3_v1_0', 'usart_v2/USART'),
    ('.*:USART:sci3_v1_1', 'usart_v2/USART'),
    #('.*:USART:sci3_v1_2', 'usart_v3/USART'),
    #('.*:USART:sci3_v2_0', 'usart_v3/USART'),
    #('.*:USART:sci3_v2_1', 'usart_v3/USART'),
]

def match_peri(peri):
    for r, block in perimap:
        if re.match(r, peri):
            return block
    return None

def parse_headers():
    os.makedirs('sources/headers_parsed', exist_ok=True)
    print('loading headers...')
    for f in glob('sources/headers/*.h'):
        #if 'stm32f4' not in f: continue
        ff = f.removeprefix('sources/headers/')
        ff = ff.removesuffix('.h')

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
        if 'STM32MP' in f: continue
        print(f)

        r = xmltodict.parse(open(f, 'rb'))['Mcu']

        package_names = expand_name(r['@RefName'])
        package_rams = r['Ram']
        package_flashs = r['Flash']
        if type(package_rams) != list: package_rams = [package_rams]*len(package_names)
        if type(package_flashs) != list: package_flashs = [package_flashs]*len(package_names)
        for package_i, package_name in enumerate(package_names):
            chip_name = chip_name_from_package_name(package_name)
            flash = int(package_flashs[package_i])
            ram = int(package_rams[package_i])
            gpio_af = next(filter(lambda x: x['@Name'] == 'GPIO', r['IP']))['@Version'].removesuffix('_gpio_v1_0')
            
            if chip_name not in chips:
                chips[chip_name] = OrderedDict({
                    'name': chip_name,
                    'flash': flash,
                    'ram': ram,
                    'gpio_af': gpio_af,
                    'packages': [],
                    'peripherals': {},
                    #'peripherals': peris,
                    #'interrupts': h['interrupts'],
                })

            chips[chip_name]['packages'].append(OrderedDict({
                'name': package_name,
                'package': r['@Package'],
            }))

            # Some packages have some peripehrals removed because the package had to
            # remove GPIOs useful for that peripheral. So we merge all peripherals from all packages.
            peris = chips[chip_name]['peripherals']
            for ip in r['IP']:
                pname = ip['@InstanceName']
                pkind = ip['@Name']+':'+ip['@Version']
                pkind = pkind.removesuffix('_Cube')

                if pname == 'SYS': pname = 'SYSCFG'
                if pname in FAKE_PERIPHERALS: continue
                peris[pname] = pkind

    for chip_name, chip in chips.items():
        h = find_header(chip_name)
        if h is None:
            raise Exception("missing header for {}".format(chip_name))
        h = headers_parsed[h]

        chip['interrupts'] = h['interrupts']

        peris = {}
        for pname, pkind in chip['peripherals'].items():
            addr = h['defines'].get(pname)
            if addr is None: continue

            p = OrderedDict({
                'addr': addr,
                'kind': pkind,
            })
            if block := match_peri(pname+':'+pkind):
                p['block'] = block
            peris[pname] = p
        chip['peripherals'] = peris
            
        with open('data/chips/'+chip_name+'.yaml', 'w') as f:
            f.write(yaml.dump(chip))

def parse_gpio_af():
    os.makedirs('data/gpio_af', exist_ok=True)
    for f in glob('sources/cubedb/mcu/IP/GPIO-*_gpio_v1_0_Modes.xml'):
        if 'STM32F1' in f: continue

        ff = f.removeprefix('sources/cubedb/mcu/IP/GPIO-')
        ff = ff.removesuffix('_gpio_v1_0_Modes.xml')
        print(ff)

        pins = {}

        r = xmltodict.parse(open(f, 'rb'))
        for pin in r['IP']['GPIO_Pin']:
            pin_name = pin['@Name']

            # Blacklist non-pins
            if pin_name == 'PDR_ON': continue

            # Cleanup pin name
            pin_name = pin_name.split('/')[0]
            pin_name = pin_name.split('-')[0]
            pin_name = pin_name.split(' ')[0]
            pin_name = pin_name.split('_')[0]
            pin_name = pin_name.split('(')[0]
            pin_name = pin_name.removesuffix('OSC32')
            pin_name = pin_name.removesuffix('BOOT0')

            # Extract AFs
            afs = {}
            for signal in children(pin, 'PinSignal'):
                func = signal['@Name']
                afn = int(signal['SpecificParameter']['PossibleValue'].split('_')[1].removeprefix('AF'))
                afs[func] = afn
            
            pins[pin_name] = afs

        with open('data/gpio_af/'+ff+'.yaml', 'w') as f:
            f.write(yaml.dump(pins))

parse_gpio_af()
parse_headers()
parse_chips()
