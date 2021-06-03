import xmltodict
import yaml
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
    y = yaml.load(f, Loader=yaml.SafeLoader)
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
            if name == 'FLASH_SIZE':
                continue
            val = val.split('/*')[0].strip()
            val = parse_value(val, defines)
            defines[name] = val

    return {
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
    # ('.*:USART:sci3_v1_2', 'usart_v3/USART'),
    # ('.*:USART:sci3_v2_0', 'usart_v3/USART'),
    # ('.*:USART:sci3_v2_1', 'usart_v3/USART'),
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
    ('.*:I2C:i2c2_v1_1F7', 'i2c_v2/I2C'),
    ('.*:DAC:dacif_v2_0', 'dac_v2/DAC'),
    ('STM32F4.*:SYS:.*', 'syscfg_f4/SYSCFG'),
    ('STM32L4.*:SYS:.*', 'syscfg_l4/SYSCFG'),
    ('STM32L0.*:SYS:.*', 'syscfg_l0/SYSCFG'),
    ('STM32H7.*:SYS:.*', 'syscfg_h7/SYSCFG'),
    ('STM32L0.*:RCC:.*', 'rcc_l0/RCC'),
    ('STM32L4.*:RCC:.*', 'rcc_l4/RCC'),
    ('STM32F4.*:RCC:.*', 'rcc_f4/RCC'),
    ('.*:STM32L0_dbgmcu_v1_0', 'dbg_l0/DBG'),
    ('.*:STM32L0_crs_v1_0', 'crs_l0/CRS'),
    ('.*SDMMC:sdmmc2_v1_0', 'sdmmc_v2/SDMMC'),
    ('.*:STM32H7_rcc_v1_0', 'rcc_h7/RCC'),
    ('.*:STM32H7_pwr_v1_0', 'pwr_h7/PWR'),
    ('.*:STM32H7_flash_v1_0', 'flash_h7/FLASH'),
    ('.*:STM32H7_dbgmcu_v1_0', 'dbgmcu_h7/DBGMCU'),
    ('.*TIM\d.*:gptimer.*', 'timer_v1/TIM_GP16'),
]

rng_clock_map = [
    ('STM32L0.*:RNG:.*', 'AHB'),
    ('STM32L4.*:RNG:.*', 'AHB2'),
    ('STM32F4.*:RNG:.*', 'AHB2'),
    ('STM32H7.*:RNG:.*', 'AHB2'),
]


def match_peri(peri):
    for r, block in perimap:
        if re.match(r, peri):
            return block
    return None

def find_af(gpio_af, peri_name, pin_name, signal_name):
     if gpio_af in af:
         if pin_name in af[gpio_af]:
             if peri_name + '_' + signal_name in af[gpio_af][pin_name]:
                 return af[gpio_af][pin_name][peri_name + '_' + signal_name]
     return None
              
def match_rng_clock(rcc):
    for r, clock in rng_clock_map:
        if re.match(r, rcc):
            return clock
    return None

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

            rcc = next(filter(lambda x: x['@Name'] == 'RCC', r['IP']))['@Version']
            rcc = removesuffix(rcc, '-rcc_v1_0')
            rcc = removesuffix(rcc, '_rcc_v1_0')

            core = r['Core']
            # multicores have a list here. Just keep the first, to not break the schema.
            if isinstance(core, list):
                core = core[0]

            if chip_name not in chips:
                chips[chip_name] = OrderedDict({
                    'name': chip_name,
                    'family': r['@Family'],
                    'line': r['@Line'],
                    'core': core,
                    'flash': flash,
                    'ram': ram,
                    'gpio_af': gpio_af,
                    'rcc': rcc,  # temporarily stashing it here
                    'packages': [],
                    'peripherals': {},
                    'pins': {},
                    # 'peripherals': peris,
                    # 'interrupts': h['interrupts'],
                })

            chips[chip_name]['packages'].append(OrderedDict({
                'name': package_name,
                'package': r['@Package'],
            }))

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
                peris[pname] = pkind
                pins[pname] = []

            for pin in r['Pin']:
                pin_name = pin['@Name']
                pin_name = pin_name.split(' ', 1)[0]
                pin_name = pin_name.split('-', 1)[0]
                if 'Signal' in pin:
                    signals = []
                    if not type(pin['Signal']) is list:
                        signals.append( pin['Signal'] )
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
               
                        pins[peri_name].append( entry )


    for chip_name, chip in chips.items():
        print(f'* processing chip {chip_name}')
        rcc = chip['rcc']
        del chip['rcc']

        h = find_header(chip_name)
        if h is None:
            raise Exception("missing header for {}".format(chip_name))
        h = headers_parsed[h]

        chip['interrupts'] = h['interrupts']

        peris = {}
        for pname, pkind in chip['peripherals'].items():
            addr = h['defines'].get(pname)
            if addr is None:
                continue

            p = OrderedDict({
                'address': addr,
                'kind': pkind,
            })

            if pname in clocks[rcc]:
                p['clock'] = clocks[rcc][pname]
            # else:
                #print( f'peri {pname} -> no clock')

            if block := match_peri(chip_name+':'+pname+':'+pkind):
                p['block'] = block

            #if block is not None and block.startswith("dac_"):
                #peri_pins = []
                #dac_out_pins = find_signal_pins(chip, pname + "_OUT([0-9]+)")
                #for (out_pin, m) in dac_out_pins:
                    #peri_pins.append( OrderedDict({ 
                        #"pin": out_pin,
                        #"function": "Ch" + m[1],
                    #}) )

            if pname in chip['pins']:
                if len(chip['pins'][pname]) > 0:
                    p['pins'] = chip['pins'][pname]

            # RNG Clock definitions are not easily determined, so lookup in mapping
            if block is not None and block.startswith("rng_"):
                if (clock := match_rng_clock(chip_name+':'+pname+':'+pkind)) != None:
                    p['clock'] = clock

            peris[pname] = p


        # Handle GPIO specially.
        for p in range(20):
            port = 'GPIO' + chr(ord('A')+p)
            if addr := h['defines'].get(port + '_BASE'):
                block = 'gpio_v2/GPIO'
                if chip['family'] == 'STM32F1':
                    block = 'gpio_v1/GPIO'

                p = OrderedDict({
                    'address': addr,
                    'block': block,
                })
                peris[port] = p
        # Handle DMA specially.
        for dma in ('DMA1', "DMA2"):
            if addr := h['defines'].get(dma + '_BASE'):
                block = 'dma_v1/DMA'
                if chip['family'] in ('STM32F4', 'STM32F7', 'STM32H7'):
                    block = 'dma_v2/DMA'

                p = OrderedDict({
                    'address': addr,
                    'block': block,
                })
                peris[dma] = p

        # EXTI is not in the cubedb XMLs
        if addr := h['defines'].get('EXTI_BASE'):
            peris['EXTI'] = OrderedDict({
                'address': addr,
                'kind': 'EXTI',
                'block': 'exti_v1/EXTI',
            })

        # FLASH is not in the cubedb XMLs
        if addr := h['defines'].get('FLASH_R_BASE'):
            kind = 'FLASH:' + chip_name[:7] + '_flash_v1_0'
            flash_peri = OrderedDict({
                'address': addr,
                'kind': kind,
            })
            if block := match_peri(kind):
                flash_peri['block'] = block
            peris['FLASH'] = flash_peri

        # DBGMCU is not in the cubedb XMLs
        if addr := h['defines'].get('DBGMCU_BASE'):
            kind = 'DBGMCU:' + chip_name[:7] + '_dbgmcu_v1_0'
            dbg_peri = OrderedDict({
                'address': addr,
                'kind': kind,
            })
            if block := match_peri(kind):
                dbg_peri['block'] = block
            peris['DBGMCU'] = dbg_peri

        # CRS is not in the cubedb XMLs
        if addr := h['defines'].get('CRS_BASE'):
            kind = 'CRS:' + chip_name[:7] + '_crs_v1_0'
            crs_peri = OrderedDict({
                'address': addr,
                'kind': kind,
            })
            if block := match_peri(kind):
                crs_peri['block'] = block
            peris['CRS'] = crs_peri
        chip['peripherals'] = peris

        # remove all pins from the root of the chip before emitting.
        del chip['pins']

        with open('data/chips/'+chip_name+'.yaml', 'w') as f:
            f.write(yaml.dump(chip))


af = {}

def parse_gpio_af():
    os.makedirs('data/gpio_af', exist_ok=True)
    for f in glob('sources/cubedb/mcu/IP/GPIO-*_gpio_v1_0_Modes.xml'):
        if 'STM32F1' in f:
            continue

        ff = removeprefix(f, 'sources/cubedb/mcu/IP/GPIO-')
        ff = removesuffix(ff, '_gpio_v1_0_Modes.xml')
        print(ff)

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

        with open('data/gpio_af/'+ff+'.yaml', 'w') as f:
            f.write(yaml.dump(pins))

        af[ff] = pins

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


parse_gpio_af()
parse_headers()
parse_clocks()
parse_chips()
