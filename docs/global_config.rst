Configuration
=============

.. note::
    Configuration of Sysinspect

Setup
-----

Configuration files can be in three location and are searched in the following order:

1. Current directory from which the app was launched: ``sysinspect.conf``.
2. "Dot-file" in the current user's home ``~/.sysinspect``
3. As ``/etc/sysinspect.conf``.

Synopsis
--------

Configuration file supports the following format:

.. code-block:: text
    :caption: Configuration Synopsis

    config:
      <key>: <value>

``modules``
^^^^^^^^^^^

Section ``modules`` defines the root of built-in modules:

.. code-block:: yaml

    modules: /opt/sysinspect/modules
