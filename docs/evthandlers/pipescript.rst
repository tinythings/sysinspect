Pipe Script
===========

.. note::

    This document explains how to use **pipescript** event handler.

Synopsis
--------

.. code-block:: text
    :caption: Initialisation

    handler:
        - pipescript

*Pipescript* handler is used to pipe action's response through any script, using STDIN.
User can define what to do with this information further.

.. important::

    This handler will react only if action contains return code ``0``, i.e. handler has
    a proper response data structure. Otherwise handler will skip the process and
    will log an error.

Options
-------

``program``
^^^^^^^^^^

    A full command line what needs to be called in event of writing STDIN to the program. Example:

    .. code-block:: yaml
        :caption: Program definition

        program: "/path/to/my/script.pl --some=argument --quiet"

``quiet``
^^^^^^^^^^^

    **Optional.** Mute logging. Example:

    .. code-block:: yaml
        :caption: Mute logging

        quiet: true

``format``
^^^^^^^^^^

In what format output needs to be sent to the target program. Options:

    - ``yaml``
    - ``json``

Example:

    .. code-block:: yaml
        :caption: Format definition

        format: json # or "yaml"

Example
-------

.. code-block:: yaml
    :caption: Setup example

    events:
      # React only on action-wise successful events
      $/$/$/0:

        handlers:
          pipescript

        pipescript:
          program: /opt/bin/extra-logger.pl
          quiet: false
          format: json
