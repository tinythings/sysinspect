``net.throughput``: React to Throughput Updates
===============================================

The ``net.throughput`` sensor watches interface counters and emits events when
calculated throughput changes are observed.

This sensor is backed by ``omnitrace/nettools`` and emits per-interface rate
samples derived from byte and packet counters.

Synopsis
--------

Sensor configuration as follows:

.. code-block:: text

    <id>:
        [profile]:
          - <id>

        description: <description>
        listener: net.throughput
        tag: <event name> # optional, default is net.throughput
        interval: <duration> # optional, default 3s

        args:
            locked: true|false # optional, default false (emit once until handler unlocks)

``profile``
^^^^^^^^^^^

    **Optional**

    The list of profiles to which this sensor belongs. If current Minion is attached to
    any other profile, the sensor will be inactive.

``description``
^^^^^^^^^^^^^^^

    A human-readable description of the sensor.

``listener``
^^^^^^^^^^^^

    The type of listener used by the sensor. In this case, it is ``net.throughput``.

``interval``
^^^^^^^^^^^^

    Poll interval for interface counter checks. If omitted, the sensor uses a
    conservative default of ``3s``.

``args``
^^^^^^^^

    Arguments specific to ``net.throughput``:

    - ``locked`` (optional): if ``true``, the same event is sent only once and then muted.
      It will be sent again only after your event handler explicitly releases/unlocks it.

``tag``
^^^^^^^

    An optional tag to associate with the event. If specified, the event name will include this tag,
    allowing easier identification and filtering of events.

    Event ID format:

    .. code-block:: text

        <sensor-id>|net.throughput[@tag]|updated@<iface>|0

Payload
-------

The emitted JSON envelope contains the usual ``libsensors`` metadata and a
throughput sample payload.

.. code-block:: json

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
          "tx_packets_per_sec": 12,
          "counters": {
            "iface": "eth0",
            "rx_bytes": 123456,
            "rx_packets": 100,
            "rx_errors": 0,
            "rx_drops": 0,
            "tx_bytes": 654321,
            "tx_packets": 200,
            "tx_errors": 0,
            "tx_drops": 0
          }
        }
      }
    }

Notes
-----

- This sensor emits only when at least one rate is non-zero.
- The emitted sample includes both the calculated rates and the current raw
  counters.

Example
-------

Here is an example of how to watch throughput changes:

.. code-block:: yaml

    throughput-watch:
        description: Watch interface throughput
        listener: net.throughput
        interval: 2s
        args:
            locked: false
        tag: throughput

Demo
----

See the demo material under:

.. code-block:: text

    examples/demos/net/

