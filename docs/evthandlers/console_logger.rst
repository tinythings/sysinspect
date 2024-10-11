Console Logger
==============

.. note::

    This document explains how to use **console logger** event handler.

Synopsis
--------

.. code-block:: text

    console-logger: null

Console logger is the simplest event handler, used to notify anything that is happening
during the system examination. It does not take any setup configuration and works
just "out of the box" as is.

Example
-------

.. code-block:: yaml
    :caption: Setup example

    events:
      # Capture all events
      $/$/$/$:

        handlers:
          console-logger
