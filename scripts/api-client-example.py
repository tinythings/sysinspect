import urllib.request
import json

def call_api(endpoint, data=None):
    """
    Call the Sysinspect API at the specified endpoint with the given data.
    """

    url = f"http://127.0.0.1:4202/api/v1/{endpoint}"
    req = urllib.request.Request(url, data=data, method="POST")
    req.add_header("Content-Type", "application/json")

    with urllib.request.urlopen(req) as resp:
        return json.loads(resp.read().decode())


query = json.dumps({
    "model": "cm/file-ops",
    "query": "*",
    "traits": "",
    "mid": "",
    "context": {
        "metaid": "ca375102-6184-11f0-893f-c32f44e18d2c",
        "tgt": "something"
    }
}).encode("utf-8")

out = call_api("query", data=query)
print(out)
