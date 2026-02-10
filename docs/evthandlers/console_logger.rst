**Console Logger**: Logging Events to Console
================================================

.. note::

    This document explains how to use **console logger** event handler.

Synopsis
--------

.. code-block:: text

    console-logger: null

Console logger is the simplest event handler, used to notify anything that is happening
during the system examination. By default it is setup without any options and works "as is".

Options
-------

``prefix``
^^^^^^^^^^

    **Optional.** Prefix to the logging message text. Example:

    .. code-block:: yaml
        :caption: Prefix example

        prefix: "Hello"

``concise``
^^^^^^^^^^^

    **Optional.** Set logger not to include output data from a certain module.

    .. code-block:: yaml
        :caption: Mute additional data display

        concise: true

Example
-------

.. code-block:: yaml
    :caption: Setup example

    events:
      # Capture all events
      $/$/$/$:

        handlers:
          console-logger

        console-logger:
          prefix: "Default event"
          concise: false # Also default
