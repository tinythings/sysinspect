``net.health``: React to Network Health Changes
===============================================

The ``net.health`` sensor watches active network probe results and emits events
when the health state changes.

This sensor is backed by ``omnitrace/nettools`` and evaluates configured probe
targets over a rolling window.

Synopsis
--------

Sensor configuration as follows:

.. code-block:: text

    <id>:
        [profile]:
          - <id>

        description: <description>
        listener: net.health
        tag: <event name> # optional, default is net.health
        interval: <duration> # optional, default 3s

        args:
            targets:
              - <host:port>   # required
            window: <int>      # optional, default 4
            timeout: <duration> # optional, default 2s
            latency-degraded-ms: <int> # optional, default 400
            loss-degraded-pct: <int>   # optional, default 25
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

    The type of listener used by the sensor. In this case, it is ``net.health``.

``interval``
^^^^^^^^^^^^

    Poll interval for probe rounds. If omitted, the sensor uses a conservative
    default of ``3s``.

``args``
^^^^^^^^

    Arguments specific to ``net.health``:

    - ``targets`` (**required**): list of probe targets in ``host:port`` form.
    - ``window`` (optional): rolling sample window size.
    - ``timeout`` (optional): per-probe timeout.
    - ``latency-degraded-ms`` (optional): latency threshold that marks the state degraded.
    - ``loss-degraded-pct`` (optional): loss percentage threshold that marks the state degraded.
    - ``locked`` (optional): if ``true``, the same event is sent only once and then muted.
      It will be sent again only after your event handler explicitly releases/unlocks it.

``tag``
^^^^^^^

    An optional tag to associate with the event. If specified, the event name will include this tag,
    allowing easier identification and filtering of events.

    Event ID format:

    .. code-block:: text

        <sensor-id>|net.health[@tag]|changed@<level>|0

Payload
-------

The emitted JSON envelope contains the usual ``libsensors`` metadata and a
health transition payload.

.. code-block:: json

    {
      "eid": "health-watch|net.health|changed@degraded|0",
      "sensor": "health-watch",
      "listener": "net.health",
      "data": {
        "action": "changed",
        "old": {
          "level": "Healthy",
          "avg_rtt_ms": 12,
          "loss_pct": 0,
          "successful_probes": 2,
          "total_probes": 2
        },
        "new": {
          "level": "Degraded",
          "avg_rtt_ms": 80,
          "loss_pct": 50,
          "successful_probes": 1,
          "total_probes": 2
        }
      }
    }

Notes
-----

- This sensor emits on state changes only.
- State levels are ``Healthy``, ``Degraded``, and ``Down``.
- The live backend currently uses TCP connect timing to the configured targets.

Example
-------

Here is an example of how to watch network health:

.. code-block:: yaml

    health-watch:
        description: Watch active network health
        listener: net.health
        interval: 2s
        args:
            targets:
              - 1.1.1.1:53
              - 8.8.8.8:53
            window: 3
            timeout: 500ms
            latency-degraded-ms: 150
            loss-degraded-pct: 25
            locked: false
        tag: health

Demo
----

See the demo material under:

.. code-block:: text

    examples/demos/net/

