import argparse
from store import Store

allowed_types = {
    "int": int,
    "float": float,
    "bool": bool,
}


def parse(name, type_str, initial, increment):
    type = allowed_types.get(type_str)
    if type is None:
        raise RuntimeError("unknown type '{}'".format(type_str))

    initial = type(initial)
    if increment is not None:
        increment = type(increment)
    return name, initial, increment


if __name__ == '__main__':
    prog = argparse.ArgumentParser()
    prog.add_argument("name")
    prog.add_argument("type")
    prog.add_argument("initial_value")
    prog.add_argument("--increment", "-i", default=None)

    args = prog.parse_args()

    name, initial, increment = parse(args.name, args.type, args.initial_value, args.increment)
    s = Store.load()
    s.register(name, initial, increment)
    s.save()

