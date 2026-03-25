Net Hostname Demo
=================

Purpose
-------
This demo shows the new `net.hostname` sensor from `libsensors`.

The sensor watches the system hostname through `omnitrace/nettools` and emits a
stable JSON event when the hostname changes.

Files
-----
- `sensors.cfg`
  Example sensor configuration for `net.hostname`

Config shape
------------
`net.hostname` currently supports:

- `listener: net.hostname`
- `interval`
  Poll interval for hostname sampling
- `args.locked`
  Enable duplicate suppression through the event id hub
- `tag`
  Optional listener tag, added as `@tag` in the listener id and event id

Emitted payload
---------------
The sensor emits the usual `libsensors` envelope:

```json
{
  "eid": "host-watch|net.hostname|changed@new-name|0",
  "sensor": "host-watch",
  "listener": "net.hostname",
  "data": {
    "action": "changed",
    "old": "old-name",
    "new": "new-name"
  }
}
```

How to run
----------
Load the demo config from this directory with the normal `libsensors` or
`sysinspect` sensor loading flow, then change the hostname on the machine.

Examples:

```bash
hostnamectl set-hostname host-node-01
hostname host-node-02
```

Expected result
---------------
When the hostname changes, the sensor emits one `changed` event with the old
and new hostname values.
