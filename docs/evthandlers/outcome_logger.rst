Outcome Logger
==============

.. note::

    This document explains how to use **outcome logger** event handler.

Synopsis
--------

.. code-block:: text

    outcome-logger: null

Outcome logger is one of the simplest event handlers, used to summarise the outcome
of all constraints.

Options
-------

``prefix``
^^^^^^^^^^

    Prefix to the logging message text. Example:

    .. code-block:: yaml
        :caption: Prefix example

        prefix: "Hello"

Example
-------

.. code-block:: yaml
    :caption: Setup example

    events:
      # Capture all events
      $/$/$/$:

        handlers:
          outcome-logger

        outcome-logger:
          prefix: "My constraints"
