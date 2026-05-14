``sys.net``
============

.. note::

    This document describes ``sys.net`` module usage.

Synopsis
--------

``sys.net`` provides network introspection with zero external tool
dependencies. It lists network interfaces, shows routing tables,
checks TCP and UDP port connectivity using raw system calls, and
measures ICMP ping latency by parsing the ``ping`` command output.

The module works on truly bare systems: kernel, filesystem, and
good manners. No ``coreutils``, ``busybox`` or ``iproute2`` needed
for interface and routing inspection. Port connectivity uses pure
``std::net`` (libc ``connect()`` underneath) — no ``nc``, no ``bash``.

Usage
-----

The following options are available:

  ``if-up``
    List available network interfaces that are up.

  ``route-table``
    Return the current routing table (main table).

  ``connect``
    Check if a TCP or UDP port is open on a remote host. Uses raw
    libc ``connect()`` — no external tools.

  ``ping``
    ICMP ping a host. Parses the ``ping`` command output for
    latency, packet loss, and TTL.

The following keyword arguments are available:

  ``if-list`` (type: string)
    Comma-separated list of interfaces to filter.

  ``host`` (type: string, required for connect/ping)
    Target hostname or IP address.

  ``port`` (type: int, required for connect)
    TCP or UDP port number.

  ``protocol`` (type: string, default: tcp)
    ``tcp`` or ``udp``.

  ``timeout`` (type: int, default: 3)
    Timeout in seconds.

  ``count`` (type: int, default: 3)
    Number of ping packets.

Examples
--------

Check if port 22 is open:

.. code-block:: yaml

    actions:
      check-ssh:
        module: sys.net
        bind:
          - target-host
        state:
          $:
            opts:
              - connect
            args:
              host:
                - 192.168.1.1
              port:
                - 22

Ping a host and get latency telemetry:

.. code-block:: yaml

    actions:
      ping-gateway:
        module: sys.net
        bind:
          - target-host
        state:
          $:
            opts:
              - ping
            args:
              host:
                - 8.8.8.8
              count:
                - 5

Query status of two network interfaces with routing table:

.. code-block:: yaml

    actions:
      check-interfaces:
        module: sys.net
        bind:
          - target-host
        state:
          $:
            opts:
              - if-up
              - route-table
            args:
              if-list:
                - eth0,eth1

Quick Test
----------

Check if port 22 is open on localhost:

.. code-block:: sh

    echo '{"options":["connect"],"arguments":{"host":"127.0.0.1","port":22}}' | target/debug/net | jq .

Ping localhost:

.. code-block:: sh

    echo '{"options":["ping"],"arguments":{"host":"127.0.0.1","count":1}}' | target/debug/net | jq .

Returning Data
--------------

``connect``
    Returns port state and latency.

    .. code-block:: json

        {
          "retcode": 0,
          "message": "Port 22/tcp on 192.168.1.1 is open (1ms)",
          "data": {
            "host": "192.168.1.1",
            "port": 22,
            "protocol": "tcp",
            "open": true,
            "latency_ms": 1
          }
        }

``ping``
    Returns ICMP statistics.

    .. code-block:: json

        {
          "retcode": 0,
          "message": "Ping to 8.8.8.8: 3 sent, 3 received, 0.0% loss",
          "data": {
            "host": "8.8.8.8",
            "sent": 3,
            "received": 3,
            "loss_pct": 0.0,
            "rtt_min": 10.5,
            "rtt_avg": 12.3,
            "rtt_max": 14.1
          }
        }

``if-up`` / ``route-table``
    Interface and route data (unchanged from v0.2.0).
