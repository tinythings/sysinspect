``sys.service``
================

.. note::

    This document describes ``sys.service`` module usage.

Synopsis
--------

The ``sys.service`` module manages operating-system services across
FreeBSD, OpenBSD, NetBSD, Linux, macOS, Solaris and Android. It detects
whichever init system is running and provides a uniform interface to
start, stop, restart, reload, enable, disable and inspect services.

All service-manager knowledge lives in YAML, shipped inside the binary.
A user-supplied ``sys.service.yaml`` library file can override or extend
definitions without recompilation.

Every operation returns structured telemetry suitable for model-level
constraints and OpenTelemetry forwarding.

Usage
-----

  ``check`` — Pure inspection, always retcode 0, full telemetry payload.
  ``info``  — Rich selectable fields for model constraints (systemd:
             load_state, active_state, sub_state, unit_path, description).
  ``status``— retcode 0 when running, 1 when not.
  ``start`` / ``stop`` / ``restart`` / ``reload`` / ``enable`` / ``disable``
           — Mutation operations, all returning telemetry.
  ``dry-run`` — Print the resolved command without executing.

Argument: ``name`` (string, required) — service name.

Supported Init Systems
----------------------

+------------------+---------------------------------------------------+
| Platform         | Detected Manager(s)                               |
+==================+===================================================+
| FreeBSD          | rc.d via ``service(8)``                           |
+------------------+---------------------------------------------------+
| OpenBSD          | ``rcctl``                                         |
+------------------+---------------------------------------------------+
| NetBSD           | rc.d via ``service(8)``                           |
+------------------+---------------------------------------------------+
| Linux            | systemd, OpenRC, runit, s6, SysV, Busybox         |
+------------------+---------------------------------------------------+
| macOS            | ``launchctl``                                     |
+------------------+---------------------------------------------------+
| Solaris          | SMF via ``svcadm``                                |
+------------------+---------------------------------------------------+
| Android          | ``setprop ctl.start`` / ``ctl.stop``              |
+------------------+---------------------------------------------------+

Quick Test
----------

Inspect sshd and get full telemetry:

.. code-block:: sh

    echo '{"options":["check"],"arguments":{"name":"sshd"}}' | target/debug/service | jq .

    # Output:
    # {
    #   "retcode": 0,
    #   "message": "Service 'sshd' is running",
    #   "data": {
    #     "name": "sshd",
    #     "running": true,
    #     "manager": "linux-systemd",
    #     "pids": [1234],
    #     "exit_code": 0
    #   }
    # }

Rich info with selectable fields:

.. code-block:: sh

    echo '{"options":["info"],"arguments":{"name":"sshd"}}' | target/debug/service | jq .data

Dry-run to see what command would be executed:

.. code-block:: sh

    echo '{"options":["restart","dry-run"],"arguments":{"name":"nginx"}}' | target/debug/service | jq .
