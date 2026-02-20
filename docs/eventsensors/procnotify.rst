``procnotify``: React to File System Events
=================================================

The ``procnotify`` sensor uses the procnotify library to monitor file system events on specified paths.
It can detect events such as file creation, deletion, and more.
This sensor is useful for monitoring specific files and directories for changes.

Synopsis
--------

Sensor configuration as follows:

.. code-block::

    <id>:
        [profile]:
          - <id>
        description: <description>
        listener: procnotify
        opts:
            - <process event> # created | terminated | etc.
        args:
            path: <path>
        tag: <event name> # optional, default is procnotify

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

    The type of listener used by the sensor. In this case, it is ``procnotify``.

``opts``
^^^^^^^^^^

    A list of process events to monitor. Possible values include:

    - ``appeared``: Triggered when a process is created
    - ``disappeared``: Triggered when a process is terminated
    - ``missing``: Triggered when a process that was not detected at all

     If not specified, the sensor will monitor all events (i.e., both appearance and disappearance).

``args``
^^^^^^^^^^
    Arguments specific to the listener. For the ``procnotify`` sensor, the following argument is required:

    - ``process``: The name of the process to monitor.
    - ``emit-on-start``: Optional argument to specify whether to emit an event immediately upon starting the sensor if the process is already present. Default is false.

     Example:

    .. code-block:: yaml

        args:
            process: bash
            start_emit: true

``tag``
^^^^^^^^^^

    An optional tag to associate with the event. If specified, the event name will include this tag,
    allowing for easier identification and filtering of events. Example:

    .. code-block:: yaml

        tag: my-tag

    In case event is defined as ``some-id`` watching some process, say ``bash``, this results
    to the following event name:

    .. code-block:: text

        some-id|procnotify@my-tag|created@bash|0

Example
-------

Here is an example of how to use the ``procnotify`` sensor to monitor a process for appearance events:

.. code-block:: yaml

    ssh_config_change:
        description: Monitor SSH configuration changes
        listener: procnotify
        opts:
            - appeared
        args:
            process: bash

        # If defined, an extra tag will be added to the event name:
        # ssh_config_change|procnotify@my-tag|appeared@bash|0
        tag: my-tag

