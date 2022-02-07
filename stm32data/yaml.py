import yaml

try:
    from yaml import CSafeLoader as SafeLoader
except ImportError:
    from yaml import SafeLoader

try:
    from yaml import CDumper as Dumper
except ImportError:
    from yaml import Dumper


def load(*args, **kwargs):
    return yaml.load(*args, Loader=SafeLoader, **kwargs)


def dump(*args, **kwargs):
    return yaml.dump(*args, Dumper=Dumper, sort_keys=False, **kwargs)
