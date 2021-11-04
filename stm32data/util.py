
def removeprefix(value: str, prefix: str) -> str:
    if value.startswith(prefix):
        return value[len(prefix):]
    else:
        return value[:]


def removesuffix(value: str, suffix: str, /) -> str:
    if value.endswith(suffix):
        return value[:-len(suffix)]
    else:
        return value[:]
