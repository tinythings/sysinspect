import sys

from nopykit import proof_value


doc = {
    "name": "nopyproof",
    "version": "0.1.0",
    "author": "SysInspect Demo",
    "description": "Proof module that imports a helper package from runtime site-packages.",
    "arguments": [
        {"name": "a", "type": "number", "required": False, "description": "First input"},
        {"name": "b", "type": "number", "required": False, "description": "Second input"},
    ],
    "returns": {
        "description": "Structured proof that helper imports worked.",
        "sample": {"import_ok": True, "proof": 12, "python_runtime": "rustpython"},
    },
}


def run(req):
    args = req.get("args", {})
    a = args.get("a", 2)
    b = args.get("b", 5)
    impl = getattr(getattr(sys, "implementation", None), "name", "python")
    return {
        "import_ok": True,
        "a": a,
        "b": b,
        "proof": proof_value(a, b),
        "python_runtime": impl,
        "message": f"Python helper import succeeded inside {impl} without requiring host python3.",
    }
