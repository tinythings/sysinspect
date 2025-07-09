## `sys.meminfo` module

This is a simple module to fetch the memory information from a Linux device.

## Building

Gnu Make is required. Then:

1. `make setup`
2. `make`

You should end up with a dynamic binary for your current platform in `bin/<target>/meminfo`.
For more options: `make <TAB><TAB>`...

## Usage

Build it, then `cd` to `bin`, find your binary there and run it like so:

	echo '{"opts": ["free", "avail"], "args": {"unit": "gb"}}' | ./meminfo | jq

You should have something like that as an output:

```json
{
  "data": {
    "changed": true,
    "mem-available": 50.08054733276367,
    "mem-free": 29.49517059326172,
    "unit": "gb"
  },
  "message": "Data has been collected successfully",
  "retcode": 0
}
```
