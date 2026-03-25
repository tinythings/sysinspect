``net.iface``: React to Network Interface Events
==================================================

The ``net.iface`` sensor watches network interface/link/address changes and emits
events for add/remove and up/down transitions.

Synopsis
--------

Sensor configuration as follows:

.. code-block:: text

    <id>:
        [profile]:
          - <id>
        description: <description>
        listener: net.iface
        opts:
            - <iface event> # iface-added | iface-removed | link-up | link-down | addr-added | addr-removed
        args:
            locked: true|false # optional, default false (emit once until handler unlocks)
        tag: <event name> # optional, default is net.iface

``opts``
^^^^^^^^

    A list of interface events to monitor:

    - ``iface-added``
    - ``iface-removed``
    - ``link-up``
    - ``link-down``
    - ``addr-added``
    - ``addr-removed``

    If omitted, all events are monitored.

``args``
^^^^^^^^

    Arguments specific to ``net.iface``:

    - ``locked`` (optional): if ``true``, the same event is sent only once and then muted.
      It will be sent again only after your event handler explicitly releases/unlocks it.

``tag``
^^^^^^^

    Event ID format:

    .. code-block:: text

        <sensor-id>|net.iface[@tag]|<action>@<ifname>|0

Example
-------

.. code-block:: yaml

    interfaces:
        description: Track interface state transitions
        listener: net.iface
        opts:
            - link-up
            - link-down
            - addr-added
            - addr-removed
        args:
            locked: false
        tag: netif
