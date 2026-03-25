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

## Net Wi-Fi Demo

### Purpose

This demo also includes the new `net.wifi` sensor from `libsensors`.

The sensor watches Wi-Fi state through `omnitrace/nettools` and emits stable
JSON events for connect, disconnect, and change transitions.

### Files

- `sensors.cfg` example sensor configuration for `net.wifi`

### Config Shape

`net.wifi` currently supports:

- `listener: net.wifi`
- `interval` poll interval for Wi-Fi sampling
- `opts` Wi-Fi transition filters:
  `connected`, `disconnected`, `changed`
- `args.locked` enable duplicate suppression through the event id hub
- `tag` optional listener tag, added as `@tag` in the listener id and event id

### Emitted Payload

The sensor emits the usual `libsensors` envelope.

For example:

```json
{
  "eid": "wifi-watch|net.wifi|connected@wlan0|0",
  "sensor": "wifi-watch",
  "listener": "net.wifi",
  "data": {
    "action": "connected",
    "wifi": {
      "iface": "wlan0",
      "connected": true,
      "link_quality": 42.0,
      "signal_level_dbm": -61.0,
      "noise_level_dbm": -95.0,
      "ssid": "garage-ap",
      "bssid": "aa:bb:cc:dd:ee:ff"
    }
  }
}
```

### How To Run

Load the demo config from this directory with the normal `libsensors` or
`sysinspect` sensor loading flow, then move between Wi-Fi conditions, roam, or
connect and disconnect the interface.

### Expected Result

When Wi-Fi state changes, the sensor emits one of:

- `connected`
- `disconnected`
- `changed`

## Net Throughput Demo

### Purpose

This demo also includes the new `net.throughput` sensor from `libsensors`.

The sensor watches interface counters through `omnitrace/nettools` and emits
stable JSON events with calculated byte and packet rates.

### Files

- `sensors.cfg` example sensor configuration for `net.throughput`

### Config Shape

`net.throughput` currently supports:

- `listener: net.throughput`
- `interval` poll interval for throughput sampling
- `args.locked` enable duplicate suppression through the event id hub
- `tag` optional listener tag, added as `@tag` in the listener id and event id

### Emitted Payload

The sensor emits the usual `libsensors` envelope.

For example:

```json
{
  "eid": "throughput-watch|net.throughput|updated@eth0|0",
  "sensor": "throughput-watch",
  "listener": "net.throughput",
  "data": {
    "action": "updated",
    "sample": {
      "iface": "eth0",
      "interval_ms": 1000,
      "rx_bytes_per_sec": 1024,
      "tx_bytes_per_sec": 2048,
      "rx_packets_per_sec": 10,
      "tx_packets_per_sec": 12
    }
  }
}
```

### How To Run

Load the demo config from this directory with the normal `libsensors` or
`sysinspect` sensor loading flow, then generate traffic on one or more
interfaces.

Examples:

```bash
curl -I https://example.com
ping -c 5 1.1.1.1
scp somefile host:/tmp/
```

### Expected Result

When interface counters change, the sensor emits `updated` events with
calculated byte and packet rates for that interface.

## Net Health Demo

### Purpose

This demo also includes the new `net.health` sensor from `libsensors`.

The sensor watches active probe health through `omnitrace/nettools` and emits
stable JSON events when health changes between `Healthy`, `Degraded`, and
`Down`.

### Files

- `sensors.cfg` example sensor configuration for `net.health`

### Config Shape

`net.health` currently supports:

- `listener: net.health`
- `interval` poll interval for probe rounds
- `args.targets` required probe targets in `host:port` form
- `args.window` rolling sample window size
- `args.timeout` per-probe timeout
- `args.latency-degraded-ms` degraded latency threshold
- `args.loss-degraded-pct` degraded loss threshold
- `args.locked` enable duplicate suppression through the event id hub
- `tag` optional listener tag, added as `@tag` in the listener id and event id

### Emitted Payload

The sensor emits the usual `libsensors` envelope.

For example:

```json
{
  "eid": "health-watch|net.health|changed@degraded|0",
  "sensor": "health-watch",
  "listener": "net.health",
  "data": {
    "action": "changed",
    "old": {
      "level": "Healthy",
      "avg_rtt_ms": 12,
      "loss_pct": 0
    },
    "new": {
      "level": "Degraded",
      "avg_rtt_ms": 180,
      "loss_pct": 50
    }
  }
}
```

### How To Run

Load the demo config from this directory with the normal `libsensors` or
`sysinspect` sensor loading flow, then disturb network connectivity or
introduce latency and packet loss against the configured targets.

### Expected Result

When health changes, the sensor emits `changed` with the old and new health
state.
