import xmltodict
import yaml
import re
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

headers = []
headers_parsed = {}

def find_header(model):
    r = ''
    for x in re.findall('(\\([^)]+\\)|.)', model):
        r += '['+''.join(re.findall('[0-9A-Z]', x))+'x]'
    res = []
    for h in headers:
        m = re.match(r, h+'xxxxxxx', re.IGNORECASE)
        if m:
            res.append(h)

    if len(res) == 2:
        res.sort()
        if res[0].endswith('xd') and res[1].endswith('xdx'):
            if model.endswith('X'):
                res = [res[1]]
            else:
                res = [res[0]]

    assert len(res) < 2
    if len(res) == 0:
        return None
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

        if m := re.match('([a-zA-Z0-9_]+)_IRQn += (\d+),? +/\\*!< (.*) \\*/', l):
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
    for f in glob('sources/headers/*.h'):
        #if 'stm32f4' not in f: continue
        print(f)
        ff = f.removeprefix('sources/headers/')
        ff = ff.removesuffix('.h')
        headers.append(ff)
        headers_parsed[ff] = parse_header(f)

def parse_chips():
    peris_by_family = {}
    peris_by_chip = {}
    peris_by_line = {}

    def put_peri(peris, peri, chip):
        if peri not in peris:
            peris[peri] = set()
        peris[peri].add(chip)

    for f in glob('sources/mcu/STM32*.xml'):
        if 'STM32MP' in f: continue
        #if 'STM32F4' not in f: continue

        print(f)

        r = xmltodict.parse(open(f, 'rb'))['Mcu']

        names = expand_name(r['@RefName'])
        rams = r['Ram']
        flashs = r['Flash']
        if type(rams) != list: rams = [rams]*len(names)
        if type(flashs) != list: flashs = [flashs]*len(names)
        for i,name in enumerate(names):
            flash = int(flashs[i])
            ram = int(rams[i])
            line = r['@Line']
            family = r['@Family']

            gpio_version = next(filter(lambda x: x['@Name'] == 'GPIO', r['IP']))['@Version'].removesuffix('_gpio_v1_0')
            
            h = find_header(name)
            if h is None: continue
            h = headers_parsed[h]

            peris = {}
            for ip in r['IP']:
                pname = ip['@InstanceName']
                pkind = ip['@Name']+':'+ip['@Version']
                pkind = pkind.removesuffix('_Cube')

                if pname == 'SYS': pname = 'SYSCFG'
                if pname in FAKE_PERIPHERALS: continue

                put_peri(peris_by_family, pkind, family.removeprefix('STM32'))
                put_peri(peris_by_line, pkind, line.removeprefix('STM32'))
                put_peri(peris_by_chip, pkind, name.removeprefix('STM32'))

                addr = h['defines'].get(pname)
                if addr is None: continue

                p = {}
                p['kind'] = pkind
                p['addr'] = addr
                if block := match_peri(pname+':'+pkind):
                    p['block'] = block
                peris[pname] = p
            
            interrupts = h['interrupts']

            chip = OrderedDict({
                'name': name,
                'flash': flash,
                'ram': ram,
                'gpio_af': gpio_version,
                'peripherals': peris,
                'interrupts': interrupts,
            })

            with open('data/chips/'+name+'.yaml', 'w') as f:
                f.write(yaml.dump(chip))

    peris_by_family = {k: ', '.join(sorted(v)) for k, v in peris_by_family.items()}
    peris_by_line = {k: ', '.join(sorted(v)) for k, v in peris_by_line.items()}
    peris_by_chip = {k: ', '.join(sorted(v)) for k, v in peris_by_chip.items()}
    with open('tmp/peris_by_family.yaml', 'w') as f: f.write(yaml.dump(peris_by_family, width=240))
    with open('tmp/peris_by_line.yaml', 'w') as f: f.write(yaml.dump(peris_by_line, width=240))
    with open('tmp/peris_by_chip.yaml', 'w') as f: f.write(yaml.dump(peris_by_chip, width=240))


def parse_gpio_af():
    for f in glob('sources/mcu/IP/GPIO-*_gpio_v1_0_Modes.xml'):
        if 'STM32F1' in f: continue

        ff = f.removeprefix('sources/mcu/IP/GPIO-')
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