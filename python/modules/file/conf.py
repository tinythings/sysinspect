from sysinspect import SysinspectReturn
# from syscore import MinionTraits

def main(*args, **kw) -> str:
    """
    Main function to dispatch the module.
    """

    # t = MinionTraits()
    # print(t.get("hardware.cpu.brand"))


    # Expand SysinspectReturn with your data.
    # See lib/sysinspect.py for more details.

    return str(SysinspectReturn().add_data({"hello": "world!"}))
