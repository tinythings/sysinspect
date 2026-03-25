``sys.filesystem``: React to Filesystem Events
=================================================

The ``sys.filesystem`` sensor uses the filesystem watcher backend to monitor file system events.
It can detect events such as file creation, modification, deletion, and more.
This sensor is useful for monitoring specific directories or files for changes.

Synopsis
--------

Sensor configuration as follows:

.. code-block::

    <id>:
        [profile]:
          - <id>
        description: <description>
        listener: sys.filesystem
        opts:
            - <file event> # created | changed | deleted
        args:
            path: <path>
            locked: true|false # optional, default false (emit once until handler unlocks)
        tag: <event name> # optional, default is sys.filesystem

``profile``
^^^^^^^^^^^

    **Optional**

    The list of profiles to which this sensor belongs. If current Minion is attached to
    any other profile, the sensor will be inactive.

``description``
^^^^^^^^^^^^^^^

    A human-readable description of the sensor.

``listener``
^^^^^^^^^^^^

    The type of listener used by the sensor. In this case, it is ``sys.filesystem``.

``opts``
^^^^^^^^^^

    A list of file events to monitor. Possible values include:

    - ``created``: Triggered when a file is created.
    - ``changed``: Triggered when a file is modified.
    - ``deleted``: Triggered when a file is deleted.

    If omitted, all three are monitored.

``args``
^^^^^^^^^^
    Arguments specific to the listener. For the ``sys.filesystem`` sensor, the following argument is required:

    - ``path``: The path to the file or directory to monitor.
    - ``locked`` (optional): if ``true``, the same event is sent only once and then muted.
      It will be sent again only after your event handler explicitly releases/unlocks it.

``tag``
^^^^^^^^^^

    An optional tag to associate with the event. If specified, the event name will include this tag,
    allowing for easier identification and filtering of events. Example:

    .. code-block:: yaml

        tag: my-tag

    In case event is defined as ``some-id`` listening to ``/tmp`` directory, then if ``/tmp/foobar`` file
    is created, this results to the following event name:

    .. code-block:: text

        some-id|sys.filesystem@my-tag|created@/tmp/foobar|0

Example
-------

Here is an example of how to use the ``sys.filesystem`` sensor to monitor a directory for file creation events:

.. code-block:: yaml

    ssh_config_change:
        description: Monitor SSH configuration changes
        listener: sys.filesystem
        opts:
            - changed
        args:
            path: /etc/ssh/sshd_config

        # If defined, an extra tag will be added to the event name:
        # ssh_config_change|sys.filesystem@my-tag|changed@/etc/ssh/sshd_config|0
        tag: my-tag
