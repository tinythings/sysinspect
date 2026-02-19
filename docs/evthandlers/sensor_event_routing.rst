Sensor Events Routing
=====================

.. important::

    🚨 If you define the filter format incorrectly, you won't get any reaction and the event will be ignored.
    Additionally, **no logging will be emitted** unless your system is running in debug mode.

A sensor event is a single positional string, delimited by ``|``:

.. code-block:: text

    sensor-id|listener|action[@specifier]|exit-code

Where:

* ``sensor-id`` is the ID of the sensor that emitted the event (e.g., ``my-tmp-dir`` — a check you run)
* ``listener`` is the listener ID that processed the sensor (e.g., ``fsnotify`` — the sensor plugin that runs the check)
* ``action`` is the action the sensor emits. For example, for ``fsnotify`` listener, the action is the event type like ``create``, ``modify``, ``delete``
* ``specifier`` is an optional part that can be used to further specify the event details, like a specific file path or a glob pattern. For example, ``create@/tmp/$``.
* ``exit-code`` is always ``0`` for sensor events, since they don't have a return code like actions do

An event routing is essentially defined like this:

.. code-block:: yaml

    sensors:
        tmp-watch:
            ...

    events:
        tmp-watch|some-sensor|result-action|0:
            ...

Each segment can be wildcarded using ``$``.
Example: ``tmp-watch|fsnotify|$|0``

Events for sensors are defined with the following synopsis:

.. code-block:: text

    events:
        <sensor-id>|<listener>|action[@specifier]|0:
            handlers:
              - handler-id
              - handler-id
              - ...

            handler-id:
                # handler configuration
                ...

Each event handler has its own configuration. Please refer to the documentation of each handler for more
details on how to set them up and use them effectively.
