``net.packet``: React to Network Connection Events
=================================================

The ``net.packet`` sensor monitors network socket table changes and emits events
when connections are opened or closed.

This sensor is useful for watching outbound/inbound connection activity, filtering
by protocol/endpoint patterns, and matching remote hosts via reverse DNS and
TLS SNI (for HTTPS flows).

Synopsis
--------

Sensor configuration as follows:

.. code-block:: text

    <id>:
        [profile]:
          - <id>
        description: <description>
        listener: net.packet
        opts:
            - <network event> # opened | closed
        args:
            patterns:
              - <pattern>     # required
            ignore:
              - <pattern>     # optional
            dns: true|false   # optional; true=force DNS, false/omitted=auto by patterns
            dns-ttl: <duration> # optional, default 60s
            sni-interface: <iface> # optional, Linux TLS SNI capture pinning
            locked: true|false # optional, default false (emit once until handler unlocks)
        tag: <event name> # optional, default is net.packet

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

    The type of listener used by the sensor. In this case, it is ``net.packet``.

``opts``
^^^^^^^^

    A list of network events to monitor. Possible values include:

    - ``opened``: Triggered when a new connection appears.
    - ``closed``: Triggered when an observed connection disappears.

    If omitted, both ``opened`` and ``closed`` are monitored.

``args``
^^^^^^^^

    Arguments specific to ``net.packet``:

    - ``patterns`` (**required**): list of match patterns.
    - ``ignore`` (optional): list of patterns to exclude.
    - ``dns`` (optional): controls reverse DNS behavior.
      ``true`` forces DNS lookups. ``false`` (or omitted) uses automatic behavior based on patterns.
    - ``dns-ttl`` (optional): reverse DNS cache TTL (for example ``60s``).
    - ``sni-interface`` / ``sni_interface`` (optional): network interface name for TLS SNI sniffing (for example ``eth0``).
      If omitted, all UP non-loopback interfaces are used.
    - ``locked`` (optional): if ``true``, the same event is sent only once and then muted.
      It will be sent again only after your event handler explicitly releases/unlocks it.

Pattern Types
^^^^^^^^^^^^^

    ``net.packet`` supports three pattern classes:

    - Host globs: ``*.comcast.net`` (matches remote reverse DNS/PTR hostname and, for HTTPS, TLS SNI hostname)
    - IP / IP:port globs: ``1.2.3.*`` or ``1.2.3.4:443`` (matches decoded remote IP endpoint)
    - DSL patterns: ``udp * *`` or ``tcp * 1.2.3.4:*`` (``<proto> <local> <remote>``)

    Host glob patterns auto-enable reverse DNS lookup unless ``dns`` is explicitly set.
    For HTTPS traffic (TCP/443), host matching can also use SNI names captured from TLS ClientHello.

SNI Interface Selection
^^^^^^^^^^^^^^^^^^^^^^^

    ``sni-interface`` pins TLS SNI sniffing to one interface (for example ``eth0``).
    Without this setting, ``net.packet`` sniffs on all UP non-loopback interfaces.

    Notes:

    - SNI matching applies to HTTPS/TCP 443 flows.
    - If the requested interface is not UP or not found, SNI capture is unavailable.
    - DNS/PTR matching still works independently of SNI capture.

``tag``
^^^^^^^

    An optional tag to associate with the event. If specified, the event name will include this tag,
    allowing for easier identification and filtering of events.

    Event ID format:

    .. code-block:: text

        <sensor-id>|net.packet[@tag]|<action>@<remote-ip:port>|0

Example
-------

Here is an example of how to watch selected remote domains while ignoring UDP traffic:

.. code-block:: yaml

    google:
        profile: [default]
        listener: net.packet
        opts:
            - opened
            - closed
        args:
            patterns:
                - "*.au-net.ne.jp"
                - "*.comcast.net"
                - "*.nt-isp.net"
            ignore:
                - "udp * *"
            dns-ttl: 60s
            sni-interface: eth0
        tag: internet-watch
