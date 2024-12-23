``sys.ssrun``
=============

.. note::

    This document describes ``sys.ssrun`` module usage.

Synopsis
--------

Similar to ``sys.run``, this plugin as same is intended to run any raw shell commands, but over SSH on a remote hosts.

Usage
-----

The following keyword arguments are available:

  cmd (type: string, required)
    Full command to run

  env (type: string)
    Modify the environment for the target running command

  locale (type: string)
    Set the locale for this command (default: LC_CTYPE=C
    Locale format is the following:

    .. code-block:: text

        KEY=value KEY1="value1" KEY3="value and spaces"

  host (type: string, required)
    Host to run the commands on

  port (type: int)
    Alternative SSH port, if not 22

  user (type: string, required)
    User name to the remote host

  rsakey (type: string, required)
    Path to the private RSA key, like /home/johnsmith/.ssh/id_rsa

  password (type: string, required)
    SSH password for the remote host.

    .. danger::

      *NOTE: using passwords in a clear text within the model is very discouraged. Please use RSA keys instead!*


Examples
--------

This is the basic usage:

.. code-block:: json

    "arguments": {
        "user": "sysinspect",
        "host": "192.168.1.2",
        "rsakey": "/etc/sysinspect/id_rsa",
        "cmd": "spotify --headless",
        "env": "PATH=$PATH:/opt/spotify/bin"
    }


If the app needs to run with additional environment or different locale, add those too:

.. code-block:: json

    "arguments": {
        "locale": "de_DE.UTF-8",
        "env": "FOO=foo BAR=\"value with spaces\"",
    }


Returning Data
--------------

Additional data is only STDOUT.

stdout
    This module returns just STDOUT of the program it is called. The format is "as is",
    usually line-separated by ``\n`` symbol, but the last ``\n`` is always trimmed.
    Return slot has two keys:

    - ``stdout`` — with the entire content of the program output.
    - ``cmd`` — the exact command that was running.

    Example:

.. code-block:: json

    "data": {
      "stdout": "x86_64",
      "cmd": "uname -p"
    }
