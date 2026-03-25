# Net Sensor Demos

## Net Hostname Demo

### Purpose

This demo shows the new `net.hostname` sensor from `libsensors`.

The sensor watches the system hostname through `omnitrace/nettools` and emits a
stable JSON event when the hostname changes.

### Files

- `sensors.cfg` example sensor configuration for `net.hostname`

### Config Shape

`net.hostname` currently supports:

- `listener: net.hostname`
- `interval` poll interval for hostname sampling
- `args.locked` enable duplicate suppression through the event id hub
- `tag` optional listener tag, added as `@tag` in the listener id and event id

### Emitted Payload

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

### How To Run

Load the demo config from this directory with the normal `libsensors` or
`sysinspect` sensor loading flow, then change the hostname on the machine.

Examples:

```bash
hostnamectl set-hostname host-node-01
hostname host-node-02
```

### Expected Result

When the hostname changes, the sensor emits one `changed` event with the old
and new hostname values.

## Net Route Demo

### Purpose

This demo also includes the new `net.route` sensor from `libsensors`.

The sensor watches the route table through `omnitrace/nettools` and emits
stable JSON events for route and default-route transitions.

### Files

- `sensors.cfg` example sensor configuration for `net.route`

### Config Shape

`net.route` currently supports:

- `listener: net.route`
- `interval` poll interval for route sampling
- `opts` route transition filters:
  `route-added`, `route-removed`, `route-changed`, `default-added`,
  `default-removed`, `default-changed`
- `args.locked` enable duplicate suppression through the event id hub
- `tag` optional listener tag, added as `@tag` in the listener id and event id

### Emitted Payload

The sensor emits the usual `libsensors` envelope.

For example:

```json
{
  "eid": "route-watch|net.route|route-added@10.0.0.0/24|0",
  "sensor": "route-watch",
  "listener": "net.route",
  "data": {
    "action": "route-added",
    "route": {
      "family": "Inet",
      "destination": "10.0.0.0/24",
      "gateway": "10.0.0.1",
      "iface": "eth0"
    }
  }
}
```

### How To Run

Load the demo config from this directory with the normal `libsensors` or
`sysinspect` sensor loading flow, then change routes on the machine.

Examples:

```bash
ip route add 10.20.30.0/24 via 192.168.1.1 dev eth0
ip route del 10.20.30.0/24
ip route replace default via 192.168.1.254 dev eth0
```

### Expected Result

When a route changes, the sensor emits one of:

- `route-added`
- `route-removed`
- `route-changed`
- `default-added`
- `default-removed`
- `default-changed`
