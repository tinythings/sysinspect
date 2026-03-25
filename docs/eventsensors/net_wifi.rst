``net.wifi``: React to Wi-Fi Changes
====================================

The ``net.wifi`` sensor watches Wi-Fi state and emits events when a Wi-Fi
interface appears, disappears, or changes.

This sensor is backed by ``omnitrace/nettools``. The current backend is Linux
first, and the backend split is already prepared for BSD implementations later.

Synopsis
--------

Sensor configuration as follows:

.. code-block:: text

    <id>:
        [profile]:
          - <id>

        description: <description>
        listener: net.wifi
        tag: <event name> # optional, default is net.wifi
        interval: <duration> # optional, default 3s

        opts:
            - <wifi event>

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

    The type of listener used by the sensor. In this case, it is ``net.wifi``.

``interval``
^^^^^^^^^^^^

    Poll interval for Wi-Fi checks. If omitted, the sensor uses a conservative
    default of ``3s``.

``opts``
^^^^^^^^

    A list of Wi-Fi events to monitor. Possible values include:

    - ``connected``
    - ``disconnected``
    - ``changed``

    If omitted, all three transitions are monitored.

``args``
^^^^^^^^

    Arguments specific to ``net.wifi``:

    - ``locked`` (optional): if ``true``, the same event is sent only once and then muted.
      It will be sent again only after your event handler explicitly releases/unlocks it.

``tag``
^^^^^^^

    An optional tag to associate with the event. If specified, the event name will include this tag,
    allowing easier identification and filtering of events.

    Event ID format:

    .. code-block:: text

        <sensor-id>|net.wifi[@tag]|<action>@<iface>|0

Payload
-------

The emitted JSON envelope contains the usual ``libsensors`` metadata and a
Wi-Fi specific payload.

For connect and disconnect transitions:

.. code-block:: json

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

For change transitions:

.. code-block:: json

    {
      "eid": "wifi-watch|net.wifi|changed@wlan0|0",
      "sensor": "wifi-watch",
      "listener": "net.wifi",
      "data": {
        "action": "changed",
        "old": {
          "iface": "wlan0",
          "connected": true,
          "link_quality": 42.0,
          "signal_level_dbm": -61.0,
          "noise_level_dbm": -95.0,
          "ssid": "garage-ap",
          "bssid": "aa:bb:cc:dd:ee:ff"
        },
        "new": {
          "iface": "wlan0",
          "connected": true,
          "link_quality": 36.0,
          "signal_level_dbm": -68.0,
          "noise_level_dbm": -96.0,
          "ssid": "garage-ap",
          "bssid": "aa:bb:cc:dd:ee:ff"
        }
      }
    }

Notes
-----

- The current live backend is Linux-first.
- The backend split in ``omnitrace/nettools`` is already in place so NetBSD and
  other BSD implementations can be added later without redesigning this sensor.

Example
-------

Here is an example of how to watch Wi-Fi changes:

.. code-block:: yaml

    wifi-watch:
        description: Watch Wi-Fi changes
        listener: net.wifi
        interval: 2s
        opts:
            - connected
            - disconnected
            - changed
        args:
            locked: false
        tag: wifi

Demo
----

See the demo material under:

.. code-block:: text

    examples/demos/net/

