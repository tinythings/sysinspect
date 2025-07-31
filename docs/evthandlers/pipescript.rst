Pipe Script
===========

.. note::

    This document explains how to use the **pipescript** event handler.

Overview
--------

The *pipescript* handler lets you send the output of an action to any script using STDIN (standard input). You can
decide what your script does with this information.

How It Works
------------

When an action is executed, the pipescript handler runs a script or program with the action's output as input. The
handler only runs if the action returns a code of ``0`` (indicating success). If the action fails (returns a different code),
the handler skips running your script and logs an error.

To initialise the pipescript handler, you need to add it to your configuration file inside ``events`` model section:

.. code-block:: yaml
    :caption: Initialisation

    handlers:
        - pipescript

The pipescript handler only runs if the action returns a code of ``0`` (which means it was successful). If the action
fails (returns a different code), the handler will skip running your script and log an error.

Options
-------

``program``
^^^^^^^^^^

    This is the full command line for the script or program you want to run. The handler will send the action's output
    to this program through STDIN. For example:

    .. code-block:: yaml
        :caption: Program definition

        program: "/path/to/my/script.pl --some=argument --quiet"

``quiet``
^^^^^^^^^

    **Optional.** If you set this to true, the handler will not log messages. For example:

    .. code-block:: yaml
        :caption: Mute logging

        quiet: true

``format``
^^^^^^^^^^

    This sets the format for the data sent to your script. You can choose:

    - ``yaml``
    - ``json``

    For example:

    .. code-block:: yaml
        :caption: Format definition

        format: json # or "yaml"

Example
-------

Here is an example of how to set up the pipescript handler in your configuration file:

.. code-block:: yaml
    :caption: Setup example

    events:
      # Only react to successful actions (return code 0)
      $/$/$/0:

        handlers:
          pipescript

        pipescript:
          program: /opt/bin/extra-logger.pl
          quiet: false
          format: json

Returned Data Format
--------------------

The following JSON format will be sent to the STDIN of the target program:

.. code-block:: json

    {
      "id.entity": "(entity ID)",
      "id.action": "(action ID)",
      "id.state": "(state ID)",

      // Error code, POSIX (0-255)
      "ret.code": 0,

      // List of warnings, if any
      "ret.warn": [],

      // Or any other message, depends on a module
      "ret.info": "Processing complete",

      // Raw JSON data straight from the module "as is"
      "ret.data": {},

      // Timestamp in RFC 3339 format, e.g.:
      "timestamp": "2025-07-30T12:43:04.117967023+00:00"
    }
