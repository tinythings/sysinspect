``ifacenotify``: React to Network Interface Events
==================================================

The ``ifacenotify`` sensor watches network interface/link/address changes and emits
events for add/remove and up/down transitions.

Synopsis
--------

Sensor configuration as follows:

.. code-block:: text

    <id>:
        [profile]:
          - <id>
        description: <description>
        listener: ifacenotify
        opts:
            - <iface event> # iface-added | iface-removed | link-up | link-down | addr-added | addr-removed
        args:
            locked: true|false # optional, default false (emit once until handler unlocks)
        tag: <event name> # optional, default is ifacenotify

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

    Arguments specific to ``ifacenotify``:

    - ``locked`` (optional): if ``true``, the same event is sent only once and then muted.
      It will be sent again only after your event handler explicitly releases/unlocks it.

``tag``
^^^^^^^

    Event ID format:

    .. code-block:: text

        <sensor-id>|ifacenotify[@tag]|<action>@<ifname>|0

Example
-------

.. code-block:: yaml

    interfaces:
        description: Track interface state transitions
        listener: ifacenotify
        opts:
            - link-up
            - link-down
            - addr-added
            - addr-removed
        args:
            locked: false
        tag: netif
