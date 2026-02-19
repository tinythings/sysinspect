``fsnotify``: React to Filesystem Events
=================================================

The ``fsnotify`` sensor uses the fsnotify library to monitor file system events.
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
        listener: fsnotify
        opts:
            - <file event> # created | changed | deleted
        args:
            path: <path>
        tag: <event name> # optional, default is fsnotify

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

    The type of listener used by the sensor. In this case, it is ``fsnotify``.

``opts``
^^^^^^^^^^

    A list of file events to monitor. Possible values include:

    - ``created``: Triggered when a file is created.
    - ``changed``: Triggered when a file is modified.
    - ``deleted``: Triggered when a file is deleted.

``args``
^^^^^^^^^^
    Arguments specific to the listener. For the ``fsnotify`` sensor, the following argument is required:

    - ``path``: The path to the file or directory to monitor.

``tag``
^^^^^^^^^^

    An optional tag to associate with the event. If specified, the event name will include this tag,
    allowing for easier identification and filtering of events. Example:

    .. code-block:: yaml

        tag: my-tag

    In case event is defined as ``some-id`` listening to ``/tmp`` directory, then if ``/tmp/foobar`` file
    is created, this results to the following event name:

    .. code-block:: text

        some-id|fsnotify@my-tag|created@/tmp/foobar|0

Example
-------

Here is an example of how to use the ``fsnotify`` sensor to monitor a directory for file creation events:

.. code-block:: yaml

    ssh_config_change:
        description: Monitor SSH configuration changes
        listener: fsnotify
        opts:
            - changed
        args:
            path: /etc/ssh/sshd_config

        # If defined, an extra tag will be added to the event name:
        # ssh_config_change|fsnotify@my-tag|changed@/etc/ssh/sshd_config|0
        tag: my-tag

