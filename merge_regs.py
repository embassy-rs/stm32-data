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

def block_items(origin, new):
    newarr=[]
    sorted(origin, key=item_key)
    sorted(new, key=item_key)

    for val in origin:
        for newval in new:
            if val["name"] == newval["name"] and val["byte_offset"] == newval["byte_offset"] and val["fieldset"] == newval["fieldset"]:
                newarr.append(newval)
                    
    return newarr

def reg_fields(origin, new):
    newarr=[]
    sorted(origin, key=field_key)
    sorted(new, key=field_key)
    for val in origin:
        for newval in new:
            if val["name"] == newval["name"] and val["bit_offset"] == newval["bit_offset"]:
                newarr.append(newval)
    return newarr

def merge_dicts(origin, new):
    merged={}
    for k, v in origin.items():
        if k in new:
            if type(v) is dict:
                merged[k] = merge_dicts(v, new[k])
            elif type(v) is list:
                if k == "items":
                    merged[k] = block_items(v, new[k])
                if k == "fields":
                    merged[k] = reg_fields(v, new[k])
            else:
                merged[k] = v
    return merged
        

first=True
reg_map={}
for regfile in sys.argv[1:]:
    print("Loading", regfile)
    with open(regfile, 'r') as f:
        y = yaml.load(f, Loader=yaml.SafeLoader)
        if not reg_map:
            reg_map = y
        else:
            reg_map = merge_dicts(reg_map, y)


with open('regs_merged.yaml', 'w') as f:
    f.write(yaml.dump(reg_map))
