from ast import mod
from importlib.machinery import ModuleSpec
import ansible
import json
import sys
import importlib.util

from types import ModuleType
from io import StringIO, BytesIO
from contextlib import redirect_stdout
from sysinspect import SysinspectReturn
from syscore import AnsibleBridge


class StdinWrapper:
    """
    A wrapper to add a 'buffer' attribute to BytesIO for compatibility with Ansible.
    """

    def __init__(self, data):
        self._buffer = BytesIO(data)

    @property
    def buffer(self):
        return self._buffer

    def read(self, *args, **kwargs):
        return self._buffer.read(*args, **kwargs)

    def readline(self, *args, **kwargs):
        return self._buffer.readline(*args, **kwargs)

    def __iter__(self):
        return iter(self._buffer)

    def close(self):
        self._buffer.close()


def invoke_ansible_module(module_path, args):
    """
    Call an Ansible module
    """
    spec: ModuleSpec | None = importlib.util.spec_from_file_location(
        "ansible_module", module_path
    )
    if spec is None:
        return "{}"

    module = importlib.util.module_from_spec(spec)

    # Inject a fake __main__ context
    main = ModuleType("__main__")
    main.__dict__.update(
        {"__file__": module_path, "__name__": "__main__", "sys": sys, "json": json}
    )
    sys.modules["__main__"] = main

    spec.loader.exec_module(module)
    sys.stdin = StdinWrapper(json.dumps({"ANSIBLE_MODULE_ARGS": args}).encode("utf-8"))

    if getattr(module, "main", None) is None:
        raise Exception("Action modules are not supported")

    output = StringIO()
    try:
        with redirect_stdout(output):
            module.main()
    except SystemExit as e:
        if e.code != 0:
            return {"error": f"Module exited with error code: {e.code}"}
    except Exception as e:
        return {"error": f"Module exception: {e}"}

    return json.loads(output.getvalue())


def main(*opts, **args) -> str:
    """
    Main function to dispatch the module.
    """

    if not opts:
        return str(
            SysinspectReturn(retcode=1, message="Target Ansible module not specified")
        )

    bridge = AnsibleBridge()
    modpath = bridge.builtin_path(opts[0][0])

    try:
        return str(
            SysinspectReturn().add_data(
                {"ansible": invoke_ansible_module(modpath, args)}
            )
        )
    except Exception as e:
        return str(SysinspectReturn(retcode=1, message=str(e)))
