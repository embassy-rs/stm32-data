import xmltodict
import yaml
import re
import json
import sys
import os
from collections import OrderedDict
from glob import glob

def item_key(a):
    return int(a["byte_offset"])

def field_key(a):
    return int(a["bit_offset"])

def merge_block(origin, new):
    for newval in new:
        found = False
        for val in origin:
            if val["name"] == newval["name"] and val["byte_offset"] == newval["byte_offset"]:
                found = True
        if not found:
            origin.append(newval)
    origin.sort(key=item_key)

def merge_fields(origin, new):
    for newval in new:
        found = False
        for val in origin:
            if val["name"] == newval["name"] and val["bit_offset"] == newval["bit_offset"]:
                found = True
        if not found:
            origin.append(newval)
    origin.sort(key=field_key)

def merge_dicts(origin, new):
    for k, v in new.items():
        if k in origin:
            if type(v) is dict:
                merge_dicts(origin[k], v)
            elif type(v) is list:
                if k == "items":
                    merge_block(origin[k], v)
                if k == "fields":
                    merge_fields(origin[k], v)
            else:
                origin[k] = v
        else:
            origin[k] = v

first=True
reg_map={}
for regfile in sys.argv[1:]:
    print("Loading", regfile)
    with open(regfile, 'r') as f:
        y = yaml.load(f, Loader=yaml.SafeLoader)
        merge_dicts(reg_map, y)


with open('regs_merged.yaml', 'w') as f:
    f.write(yaml.dump(reg_map))
