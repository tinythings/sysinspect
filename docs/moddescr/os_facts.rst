``os.facts``
=============

.. note::

    This document describes ``os.facts`` module usage.

Synopsis
--------

``os.facts`` gathers system facts — OS, kernel version, architecture,
hostname, memory sizes, CPU model and core count, uptime, and load
averages — and returns them as a flat JSON object suitable for
configuration-management targeting, monitoring, and OpenTelemetry
forwarding.

It is a minimal, no_std binary written against raw libc syscalls.
No heap allocation, no Vec, no HashMap, no serde. Just stack buffers
and ``read()``/``write()``. Release builds are ~300 KB.

On Linux, facts are read directly from ``/proc``. The module compiles
and runs on FreeBSD, OpenBSD, NetBSD, macOS, and Solaris, returning
the cross-platform fields (os, arch, hostname, kernel) and best-effort
values for the rest.

Usage
-----

  ``gather`` — Collect all facts, return a flat JSON object.
  ``get``    — Return a single fact by key (argument ``key``).
  ``list-keys`` — Return a JSON array of available fact keys.
  ``--help`` / ``-h`` — Print help from the command line or from
  the stdin JSON ``"options":["help"]``.

Facts
-----

+---------------------+--------------------------------------+-----------+
| Key                 | Description                          | Linux     |
+=====================+======================================+===========+
| ``os``              | Operating system name                | ✓ always  |
+---------------------+--------------------------------------+-----------+
| ``arch``            | CPU architecture                     | ✓ always  |
+---------------------+--------------------------------------+-----------+
| ``hostname``        | System hostname                      | ✓ always  |
+---------------------+--------------------------------------+-----------+
| ``kernel``          | Kernel release string                | ✓ always  |
+---------------------+--------------------------------------+-----------+
| ``uptime_seconds``  | System uptime in seconds             | /proc     |
+---------------------+--------------------------------------+-----------+
| ``memory_total_kb`` | Total physical memory (KB)           | /proc     |
+---------------------+--------------------------------------+-----------+
| ``memory_free_kb``  | Free physical memory (KB)            | /proc     |
+---------------------+--------------------------------------+-----------+
| ``swap_total_kb``   | Total swap space (KB)                | /proc     |
+---------------------+--------------------------------------+-----------+
| ``swap_free_kb``    | Free swap space (KB)                 | /proc     |
+---------------------+--------------------------------------+-----------+
| ``cpu_model``       | CPU model name string                | /proc     |
+---------------------+--------------------------------------+-----------+
| ``cpu_cores``       | Number of logical CPU cores          | /proc     |
+---------------------+--------------------------------------+-----------+
| ``load_1m``         | 1-minute load average                | /proc     |
+---------------------+--------------------------------------+-----------+
| ``load_5m``         | 5-minute load average                | /proc     |
+---------------------+--------------------------------------+-----------+

Quick Test
----------

Gather all facts:

.. code-block:: sh

    echo '{"options":["gather"]}' | target/release/facts | python3 -m json.tool

    # Output:
    # {
    #     "retcode": 0,
    #     "data": {
    #         "os": "linux",
    #         "arch": "x86_64",
    #         "hostname": "alien",
    #         "kernel": "5.19.0-50-generic",
    #         "uptime_seconds": "342447.19",
    #         "memory_total_kb": "32535648",
    #         "memory_free_kb": "4788896",
    #         "swap_total_kb": "15625212",
    #         "swap_free_kb": "14428668",
    #         "cpu_model": "12th Gen Intel(R) Core(TM) i7-12700H",
    #         "cpu_cores": "2",
    #         "load_1m": "1.75",
    #         "load_5m": "1.41"
    #     }
    # }

Get a single fact by key:

.. code-block:: sh

    echo '{"options":["get"],"arguments":{"key":"os"}}' | target/release/facts

    # {"retcode":0,"data":{"os":"linux"}}

List all available keys:

.. code-block:: sh

    echo '{"options":["list-keys"]}' | target/release/facts

    # {"retcode":0,"data":["os","arch","hostname",...]}

Print help:

.. code-block:: sh

    target/release/facts --help

Examples
--------

Gather facts in a model action for CM targeting:

.. code-block:: yaml

    actions:
      collect-facts:
        module: os.facts
        bind:
          - target-host
        state:
          $:
            opts:
              - gather

Check if the host has enough memory (≥ 8 GB) before installing:

.. code-block:: yaml

    constraints:
      collect-facts:
        entities:
          - $
        all:
          $:
            - fact: data(memory_total_kb)
            - more: 8000000

Returning Data
--------------

Gather returns a flat object with all available facts. Get returns a
single key-value pair. List-keys returns a string array.

.. code-block:: json

    {
      "retcode": 0,
      "data": {
        "os": "linux",
        "arch": "x86_64",
        "hostname": "alien",
        "kernel": "5.19.0-50-generic",
        "uptime_seconds": "342447.19",
        "memory_total_kb": "32535648",
        "memory_free_kb": "4788896",
        "swap_total_kb": "15625212",
        "swap_free_kb": "14428668",
        "cpu_model": "12th Gen Intel(R) Core(TM) i7-12700H",
        "cpu_cores": "2",
        "load_1m": "1.75",
        "load_5m": "1.41"
      }
    }
