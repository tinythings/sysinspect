from mathx import addmul


doc = {
    "name": "hello",
    "version": "0.1.0",
    "author": "Bo Maryniuk",
    "description": "Adds two numbers with a shared helper from site-packages.",
    "arguments": [
        {"name": "a", "type": "number", "required": True, "description": "First number"},
        {"name": "b", "type": "number", "required": True, "description": "Second number"},
    ],
    "options": [],
    "examples": [
        {
            "description": "Add 2 and 5",
            "code": 'module: py3.hello',
        }
    ],
    "returns": {
        "description": "Returns {sum=<number>}",
        "sample": {"sum": 12},
    },
}


def run(req):
    args = req.get("args", {})
    a = args.get("a", 0)
    b = args.get("b", 0)
    return {"sum": addmul(a, b)}
