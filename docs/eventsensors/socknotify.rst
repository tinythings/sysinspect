``socknotify``: React to Socket Open/Close Events
=================================================

The ``socknotify`` sensor monitors socket table changes and emits events when
sockets are opened or closed.

Synopsis
--------

Sensor configuration as follows:

.. code-block:: text

    <id>:
        [profile]:
          - <id>
        description: <description>
        listener: socknotify
        opts:
            - <socket event> # opened | closed
        args:
            patterns:
              - <glob pattern>   # optional, defaults to all
            ignore:
              - <glob pattern>   # optional
            dns: true|false      # optional, default false
            dns-ttl: <duration>  # optional, default 60s
            skip-reverse-dns: true|false # optional, default false
            locked: true|false   # optional, default false (emit once until handler unlocks)
        tag: <event name> # optional, default is socknotify

``opts``
^^^^^^^^

    A list of socket events to monitor:

    - ``opened``: Triggered when a new socket appears.
    - ``closed``: Triggered when a previously observed socket disappears.

    If omitted, both are monitored.

``args``
^^^^^^^^

    Arguments specific to ``socknotify``:

    - ``patterns`` (optional): list of glob patterns applied to the normalized tuple
      ``<proto> <local> <remote> <remote-host> <state>``.
      If omitted, all sockets are watched.
    - ``ignore`` (optional): list of exclusion glob patterns, same tuple format.
    - ``dns`` (optional): if ``true``, perform reverse DNS on remote endpoints.
    - ``dns-ttl`` (optional): reverse DNS cache TTL (for example ``60s``).
    - ``skip-reverse-dns`` / ``skip_reverse_dns`` (optional): skip DNS for local/non-routable IPs.
    - ``locked`` (optional): if ``true``, the same event is sent only once and then muted.
      It will be sent again only after your event handler explicitly releases/unlocks it.

``tag``
^^^^^^^

    Event ID format:

    .. code-block:: text

        <sensor-id>|socknotify[@tag]|<action>@<remote-ip:port>|0

Example
-------

.. code-block:: yaml

    sockets:
        description: Watch internet-facing TCP sockets
        listener: socknotify
        opts:
            - opened
            - closed
        args:
            patterns:
                - "tcp * * * ESTABLISHED"
            ignore:
                - "udp * * * *"
            dns: true
            dns-ttl: 60s
            skip-reverse-dns: true
            locked: false
        tag: net-sockets
