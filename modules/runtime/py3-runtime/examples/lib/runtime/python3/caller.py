import subprocess


def doc():
    return {
        "name": "caller",
        "version": "0.1.0",
        "author": "Bo Maryniuk",
        "description": "Executes ls -lah on a given directory and returns the output.",
        "arguments": [
            {"name": "dir", "type": "string", "required": True, "description": "Directory path to list"}
        ],
        "options": [
            {"name": "lines", "description": "Split stdout into an array of lines"}
        ],
        "examples": [
            {
                "description": "List /etc as raw output",
                "code": 'module: py3.caller',
            },
            {
                "description": "List /etc as lines",
                "code": 'module: py3.caller',
            },
        ],
        "returns": {
            "description": "Returns stdout of ls -lah <dir>",
            "sample": {"output": ["total 4.0K", "-rw-r--r-- 1 root root ..."]},
        },
    }


def run(req):
    args = req.get("args", {})
    opts = req.get("opts", [])
    path = args.get("dir", "")
    if not path:
        raise RuntimeError("argument 'dir' is required")

    proc = subprocess.run(["ls", "-lah", path], capture_output=True, text=True)
    out = proc.stdout if proc.returncode == 0 else proc.stderr
    return {
        "command": f"ls -lah {path}",
        "exit_code": proc.returncode,
        "output": [line for line in out.splitlines() if line] if "lines" in opts else out,
    }
