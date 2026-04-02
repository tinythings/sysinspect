``sys.proc``: React to Process Events
=================================================

The ``sys.proc`` sensor is watching for OS process events, such as process creation and termination.
This sensor is portable across Unix-like systems and can be used to monitor specific processes for
security, performance, or operational reasons.

Synopsis
--------

Sensor configuration as follows:

.. code-block:: text

    <id>:
        [profile]:
          - <id>

        description: <description>
        listener: sys.proc
        tag: <event name> # optional, default is sys.proc

        opts:
            - <process event> # appeared | disappeared | missing

        args:
            process:
                - <process name>
                - <process name>

            emit-on-start: true|false # optional, default false
            locked: true|false # optional, default false (emit once until handler unlocks)

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

    The type of listener used by the sensor. In this case, it is ``sys.proc``.

``opts``
^^^^^^^^^^

    A list of process events to monitor. Possible values include:

    - ``appeared``: Triggered when a process is created
    - ``disappeared``: Triggered when a process is terminated
    - ``missing``: Triggered when a process was not detected at all

     If not specified, the sensor will monitor ``appeared`` and ``disappeared``.

``args``
^^^^^^^^^^
    Arguments specific to the listener. For the ``sys.proc`` sensor, the following argument is required:

    - ``process``: list of names of the processes to monitor.
    - ``emit-on-start``: Optional argument to specify whether to emit an event immediately upon starting
      the sensor if the process is already present. Default is false.
    - ``locked`` (optional): if ``true``, the same event is sent only once and then muted.
      It will be sent again only after your event handler explicitly releases/unlocks it.

     Example:

    .. code-block:: yaml

        args:
            process:
                - bash
                - sshd
            emit-on-start: true

``tag``
^^^^^^^^^^

    An optional tag to associate with the event. If specified, the event name will include this tag,
    allowing for easier identification and filtering of events. Example:

    .. code-block:: yaml

        tag: my-tag

    In case event is defined as ``some-id`` watching some process, say ``bash``, this results
    to the following event name:

    .. code-block:: text

        some-id|sys.proc@my-tag|appeared@bash|0

Example
-------

Here is an example of how to use the ``sys.proc`` sensor to monitor a process for appearance events:

.. code-block:: yaml

    ssh_config_change:
        description: Monitor SSH configuration changes
        listener: sys.proc
        opts:
            - appeared
        args:
            process:
                - bash

        # If defined, an extra tag will be added to the event name:
        # ssh_config_change|sys.proc@my-tag|appeared@bash|0
        tag: my-tag
