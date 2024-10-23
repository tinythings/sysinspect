``sys.run``
===========

.. note::

    This document describes ``sys.run`` module usage.

Synopsis
--------

This plugin is intended to run any raw shell commands, as well as leave commands
running in the background.

Usage
-----

The following options are available:

  disown
    Leaves the program running in the background

The following keyword arguments are available:

  cmd (type: string, required)
    Full command to run

  send (type: string)
    Send uninterpolated data to the program input (STDIN)

  env (type: string)
    Modify the environment for the target running command

  locale (type: string)
    Set the locale for this command (default: LC_CTYPE=C
    Locale format is the following:

    .. code-block:: text

        KEY=value KEY1="value1" KEY3="value and spaces"


Examples
--------

This is the basic usage:

.. code-block:: json

    "arguments": {
        "cmd": "/usr/bin/my_app",
    }


If the app needs to run with additional environment or different locale:

.. code-block:: json

    "arguments": {
        "cmd": "/usr/bin/my_app",
        "locale": "de_DE.UTF-8",
        "env": "FOO=foo BAR=\"value with spaces\"",
    }


Returning Data
--------------

Additional data is only STDOUT.

stdout
    This module returns just STDOUT of the program it is called. The format is "as is",
    usually line-separated by ``\n`` symbol.

    Example:

.. code-block:: json

  {
    "stdout": "...."
  },
