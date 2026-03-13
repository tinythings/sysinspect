doc = {
    "name": "reader",
    "version": "0.1.0",
    "author": "Bo Maryniuk",
    "description": "Reads /etc/os-release and returns VERSION.",
    "arguments": [],
    "options": [],
    "examples": [
        {
            "description": "Read OS version",
            "code": '{ "module": "py3.reader", "opts": ["rt.logs"] }',
        }
    ],
    "returns": {
        "description": "Returns detected OS version",
        "sample": {"version": "12 (bookworm)"},
    },
}


def read_os_release():
    with open("/etc/os-release", "r", encoding="utf-8") as fh:
        for line in fh:
            if line.startswith("VERSION="):
                return line.split("=", 1)[1].strip().strip('"')
    return None


def run(_req):
    version = read_os_release()
    if version:
        log.info("Detected OS VERSION:", version)
    else:
        log.error("VERSION not found in /etc/os-release")
    return {"version": version}
