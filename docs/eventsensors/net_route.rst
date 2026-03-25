``net.route``: React to Route Changes
=====================================

The ``net.route`` sensor watches the local route table and emits events when
routes or the default route are added, removed, or changed.

This sensor is backed by ``omnitrace/nettools`` and keeps route watching
separate from the other network sensors.

Synopsis
--------

Sensor configuration as follows:

.. code-block:: text

    <id>:
        [profile]:
          - <id>

        description: <description>
        listener: net.route
        tag: <event name> # optional, default is net.route
        interval: <duration> # optional, default 3s

        opts:
            - <route event>

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

    The type of listener used by the sensor. In this case, it is ``net.route``.

``interval``
^^^^^^^^^^^^

    Poll interval for route checks. If omitted, the sensor uses a conservative
    default of ``3s``.

``opts``
^^^^^^^^

    A list of route events to monitor. Possible values include:

    - ``route-added``
    - ``route-removed``
    - ``route-changed``
    - ``default-added``
    - ``default-removed``
    - ``default-changed``

    If omitted, all six transitions are monitored.

``args``
^^^^^^^^

    Arguments specific to ``net.route``:

    - ``locked`` (optional): if ``true``, the same event is sent only once and then muted.
      It will be sent again only after your event handler explicitly releases/unlocks it.

``tag``
^^^^^^^

    An optional tag to associate with the event. If specified, the event name will include this tag,
    allowing easier identification and filtering of events.

    Event ID format:

    .. code-block:: text

        <sensor-id>|net.route[@tag]|<action>@<destination>|0

Payload
-------

The emitted JSON envelope contains the usual ``libsensors`` metadata and a
route-specific payload.

For add and remove transitions:

.. code-block:: json

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

For change transitions:

.. code-block:: json

    {
      "eid": "route-watch|net.route|default-changed@default|0",
      "sensor": "route-watch",
      "listener": "net.route",
      "data": {
        "action": "default-changed",
        "old": {
          "family": "Inet",
          "destination": "default",
          "gateway": "10.0.0.1",
          "iface": "eth0"
        },
        "new": {
          "family": "Inet",
          "destination": "default",
          "gateway": "10.0.0.254",
          "iface": "eth1"
        }
      }
    }

Example
-------

Here is an example of how to watch route and default-route changes:

.. code-block:: yaml

    route-watch:
        description: Watch route table changes
        listener: net.route
        interval: 2s
        opts:
            - route-added
            - route-removed
            - route-changed
            - default-changed
        args:
            locked: false
        tag: route

Demo
----

See the demo material under:

.. code-block:: text

    examples/demos/net/
