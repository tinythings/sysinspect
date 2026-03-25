``net.hostname``: React to Hostname Changes
===========================================

The ``net.hostname`` sensor watches the local system hostname and emits an
event whenever the hostname changes.

This sensor is backed by ``omnitrace/nettools`` and is intentionally narrow:
it only watches hostname transitions and keeps a stable JSON payload for event
handlers.

Synopsis
--------

Sensor configuration as follows:

.. code-block:: text

    <id>:
        [profile]:
          - <id>

        description: <description>
        listener: net.hostname
        tag: <event name> # optional, default is net.hostname
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

    The type of listener used by the sensor. In this case, it is ``net.hostname``.

``interval``
^^^^^^^^^^^^

    Poll interval for hostname checks. If omitted, the sensor uses a conservative
    default of ``3s``.

``args``
^^^^^^^^

    Arguments specific to ``net.hostname``:

    - ``locked`` (optional): if ``true``, the same event is sent only once and then muted.
      It will be sent again only after your event handler explicitly releases/unlocks it.

``tag``
^^^^^^^

    An optional tag to associate with the event. If specified, the event name will include this tag,
    allowing easier identification and filtering of events.

    Event ID format:

    .. code-block:: text

        <sensor-id>|net.hostname[@tag]|changed@<new-hostname>|0

Payload
-------

The emitted JSON envelope contains the usual ``libsensors`` metadata and a
small hostname-specific payload:

.. code-block:: json

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

Example
-------

Here is an example of how to watch hostname changes:

.. code-block:: yaml

    host-watch:
        description: Watch hostname changes
        listener: net.hostname
        interval: 2s
        args:
            locked: false
        tag: host

Demo
----

See the demo material under:

.. code-block:: text

    libsensors/demos/net/
