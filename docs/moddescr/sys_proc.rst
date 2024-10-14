``sys.proc``
============

.. note::

    This document describes ``sys.proc`` module usage.

Synopsis
--------

This is a plugin that performs various operations on a regular processes, such as
starting them, stopping, checking their presense and gathering any kind of data,
related to them.

This module can be used for information extraction, monitoring, and as well as for
configuration management purposes applying certain system state.

Usage
-----

The following options are available:

  limits
    Add limits information.

  pid
    Return process ID.

The following keyword arguments are available:

  search (type: string, required)
    Searching mask. The more precise, the better it is.

  started (type: bool)
    If specified, process will be either started or stopped.



Examples
--------

An example of JSON call, which is checking if a specific process is started,
and if it is not, an attempt is performed:

.. code-block:: json

    "arguments": {
        "search": "/usr/bin/my_app",
        "started": true
    }


Additionally to the example above, this section will add limits and PID to the common result:

.. code-block:: json

    "options": ["limits", "pid"]


Returning Data
--------------

limits
    Causes module to return a tabular data of process "limits" and "cmd" containing an actual command line. Example:

.. code-block:: json

  {
    "limits": [
        ["attribute", "soft", "hard", "units"],
        ["cpu time", -1, -1, "seconds"],
        ["processes", 126599, 126599, "processes"],
        ["open files", 1024, 524288, "files"],
      ],
    "cmd": "/usr/bin/my_app"
  },

pid
    Returns key/value of PID and its number. Example:

.. code-block:: json

  {
    "pid": 14056,
  }