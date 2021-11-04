import re
import os
import json
from glob import glob

from stm32data import yaml
from stm32data.util import removeprefix, removesuffix


headers_parsed = {}
header_map = {}
with open('header_map.yaml', 'r') as f:
    y = yaml.load(f)
    for header, chips in y.items():
        for chip in chips.split(','):
            header_map[chip.strip().lower()] = header.lower()


def get_for_chip(model):
    if header := get_header_name_for_chip(model):
        return headers_parsed[header]
    return None


def get_header_name_for_chip(model):
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
    if m := re.match('(0([1-9][0-9]*)(U))', val):
        return int(m.group(2), 10)
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
    if m := re.match('(.*)/(.*)$', val):
        return parse_value(m.group(1), defines) / parse_value(m.group(2), defines)
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
            # print("Cur core is ", cur_core, "matched", l)
            found = False
            for core in cores:
                if core == cur_core:
                    found = True
            if not found:
                cores.append(cur_core)
            # print("Switching to core", cur_core, "for", f)
        elif m := re.match('.*else.*', l):
            cur_core = "all"
            if m := re.match('.*else.*CORE_CM(\\d+)(PLUS)?.*', l):
                cur_core = "cm" + str(m.group(1))
                if m.group(2) != None:
                    cur_core += "p"
                # print("Cur core is ", cur_core, "matched", l)
            elif len(cores) > 1:
                # Pick the second core assuming we've already parsed one
                cur_core = cores[1]

            found = False
            for core in cores:
                if core == cur_core:
                    found = True
            if not found:
                cores.append(cur_core)
            # print("Switching to core", cur_core, "for", f)
        elif m := re.match('.*endif.*', l):
            # print("Switching to common core for", f)
            cur_core = "all"

        if cur_core not in irqs:
            # print("Registering new core", cur_core)
            irqs[cur_core] = {}
        if cur_core not in defines:
            defines[cur_core] = {}

        if m := re.match('([a-zA-Z0-9_]+)_IRQn += (\\d+),? +/\\*!< (.*) \\*/', l):
            # print("Found irq for", cur_core)
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
            # print("Found define for", cur_core)
            defines[cur_core][name] = val

    # print("Found", len(cores), "cores for", f)
    # print("Found", len(irqs['all']), "shared interrupts for", f)

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


parse_headers()
