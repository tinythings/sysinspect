import sys

doc = {
    "name": "nopython",
    "version": "0.1.0",
    "author": "SysInspect Demo",
    "description": "Return proof that Python code executed inside the embedded runtime.",
    "arguments": [],
    "options": [
        {"name": "rt.logs", "description": "Forward Python-side logs into SysInspect runtime logs"}
    ],
    "returns": {
        "description": "Structured proof that Python executed and imported a runtime helper package.",
        "sample": {
            "python_runtime": "rustpython",
            "python_version": "3.x",
            "python_platform": "linux",
            "python_byteorder": "little",
            "lambda_type": "function",
            "eval_result": 10,
            "import_ok": True,
            "message": "Python proof: impl=rustpython, ver=3.x, platform=linux, byteorder=little, stdlib=3, helper=nopykit.",
        },
    },
}


def runtime_identity():
    impl = getattr(sys, "implementation", None)
    impl_name = getattr(impl, "name", "python") if impl is not None else "python"
    version_info = getattr(sys, "version_info", None)
    if version_info is not None:
        version_short = f"{version_info.major}.{version_info.minor}.{version_info.micro}"
    else:
        version_short = sys.version.split()[0] if getattr(sys, "version", "") else "unknown"

    return {
        "implementation": impl_name,
        "version": getattr(sys, "version", version_short),
        "version_short": version_short,
    }


def run(_req):
    runtime = runtime_identity()
    runtime_seed = sum(ord(ch) for ch in runtime["implementation"]) + sys.version_info.major + sys.version_info.minor + sys.version_info.micro
    stdlib_imports = {
        "json": __import__("json").__name__,
        "math": __import__("math").__name__,
        "hashlib": __import__("hashlib").__name__,
    }
    lambda_type = type(lambda: 1).__name__
    comprehension_values = [n * n for n in range(sys.version_info.major + 2)]
    comprehension_sum = sum(comprehension_values)
    eval_expr = f"{sys.version_info.major} + {sys.version_info.minor} + {sys.version_info.micro}"
    eval_result = eval(compile(eval_expr, "<nopython-proof>", "eval"))
    generator_result = list(x + sys.version_info.major for x in range(3))
    helper_module = __import__("nopykit")
    try:
        int(f"{runtime['implementation']}-{sys.version_info.major}")
    except Exception as exc:
        exception_type = type(exc).__name__

    message = (
        f"Python proof: impl={runtime['implementation']}, ver={runtime['version_short']}, "
        f"platform={sys.platform}, byteorder={sys.byteorder}, stdlib={len(stdlib_imports)}, helper={helper_module.__name__}."
    )

    log.info("Collected embedded Python runtime proof")
    log.info(message)

    return {
        "python_runtime": runtime["implementation"],
        "python_version": runtime["version_short"],
        "python_full_version": runtime["version"],
        "python_platform": sys.platform,
        "python_byteorder": sys.byteorder,
        "python_version_info": {
            "major": sys.version_info.major,
            "minor": sys.version_info.minor,
            "micro": sys.version_info.micro,
        },
        "sys_argv": list(sys.argv),
        "sys_path_head": list(sys.path[:5]),
        "runtime_seed": runtime_seed,
        "lambda_type": lambda_type,
        "lambda_result": (lambda x: x + sys.version_info.minor)(sys.version_info.major),
        "comprehension_values": comprehension_values,
        "comprehension_sum": comprehension_sum,
        "eval_expression": eval_expr,
        "eval_result": eval_result,
        "generator_result": generator_result,
        "exception_type": exception_type,
        "stdlib_imports": stdlib_imports,
        "import_ok": True,
        "helper_module": helper_module.__name__,
        "helper_module_file": getattr(helper_module, "__file__", None),
        "helper_module_keys": sorted([key for key in dir(helper_module) if not key.startswith("_")])[:8],
        "message": message,
    }
