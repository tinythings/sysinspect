from sysinspect import SysinspectReturn
from syscore import MinionTraits
import os

def help() -> str:
    return """
Options:
    traits - display all minion traits
    help   - this help
    ver    - Display version
    """

def main(args, **kw) -> str:
    """
    Main function to dispatch the module.
    """

    # Expand SysinspectReturn with your data.
    # See lib/sysinspect.py for more details.
    r = SysinspectReturn()

    out = {"changed": False}

    if "help" in args:
        print(help())
        return str(r)

    if "ver" in args:
        print("Version 0.1")
        return str(r)

    if "traits" in args:
        t = MinionTraits()
        out.update(dict(map(lambda x:(x[0], x[1]), map(lambda k:(k, t.get(k)), t.list()))))

    return str(r.add_data(out))
