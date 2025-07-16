import urllib.request
import json

req = urllib.request.Request("http://127.0.0.1:4202/api/v1/health", data=b"", method="POST")
req.add_header("Content-Type", "application/json")

with urllib.request.urlopen(req) as resp:
    out = json.loads(resp.read().decode())
    print(out)
