import yaml
from collections import OrderedDict

try:
    from yaml import CSafeLoader as SafeLoader
except ImportError:
    from yaml import SafeLoader


class DecimalInt:
    def __init__(self, val):
        self.val = val


def represent_decimal_int(dumper, data):
    return dumper.represent_int(data.val)


yaml.add_representer(DecimalInt, represent_decimal_int)


class HexInt:
    def __init__(self, val):
        self.val = val


def represent_hex_int(dumper, data):
    return dumper.represent_int(hex(data.val))


yaml.add_representer(HexInt, represent_hex_int)


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


def load(*args, **kwargs):
    return yaml.load(*args, Loader=SafeLoader, **kwargs)


def dump(*args, **kwargs):
    return yaml.dump(*args, **kwargs)
