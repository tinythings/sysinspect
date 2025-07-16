import urllib.request
import json

req = urllib.request.Request("http://127.0.0.1:4202/api/v1/health", data=b"", method="POST")
req.add_header("Content-Type", "application/json")

with urllib.request.urlopen(req) as resp:
    out = json.loads(resp.read().decode())
    print(out)


query = json.dumps({"query": "cm/file-ops;*;;;metaid:ca375102-6184-11f0-893f-c32f44e18d2c,tgt:something"})
req = urllib.request.Request("http://127.0.0.1:4202/api/v1/query", data=query.encode("utf-8"), method="POST")
req.add_header("Content-Type", "application/json")

with urllib.request.urlopen(req) as resp:
    out = json.loads(resp.read().decode())
    print(out)

