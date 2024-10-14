``sys.net``
============

.. note::

    This document describes ``sys.net`` module usage.

Synopsis
--------

This plugin is intended to return information about the network details.
One particular feature of this module that it does not require any additional networking tools
installed, such as ``coreutils`` or ``busybox`` etc. It implements the functionality
on its own and can run on "bare system" with minimum possible installation.

Usage
-----

Options:

  if-up
    List of available network interfaces that are up.

  route-table
    Return current routing table with default route (main table).

Keyword arguments:

  if-stat (type: string)
    Comma-separated list of interfaces to get their status.

Examples
--------

In this example, request is constructed in order to query the status of two network interfaces.

.. code-block:: json

    "arguments": {
        "if-stat": "eth0,virbr01",
    }

Returning Data
--------------

Route information example example:

.. code-block:: json

  "route-table":
    [
      { "gateway": "192.168.1.1", "mask": "0" },
      {
        "dst": "169.254.0.0",
        "mask": "16",
        "proto": "boot",
        "scope": "link",
      },
      {
        "dst": "192.168.2.0",
        "if": "eth0",
        "mask": "24",
        "proto": "kernel",
        "scope": "link",
        "src": "192.168.1.123",
      },
    ]
