``mountnotify``: React to Mount Table Events
============================================

The ``mountnotify`` sensor monitors mount table changes and emits events when
filesystems are mounted, unmounted, or changed.

Synopsis
--------

Sensor configuration as follows:

.. code-block:: text

    <id>:
        [profile]:
          - <id>
        description: <description>
        listener: mountnotify
        opts:
            - <mount event> # mounted | unmounted | changed
        args:
            mountpoints:
              - <mountpoint path>
              - <mountpoint path>
            locked: true|false # optional, default false (emit once until handler unlocks)
        tag: <event name> # optional, default is mountnotify

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

    The type of listener used by the sensor. In this case, it is ``mountnotify``.

``opts``
^^^^^^^^

    A list of mount events to monitor. Possible values include:

    - ``mounted``: Triggered when a mountpoint appears.
    - ``unmounted``: Triggered when a mountpoint disappears.
    - ``changed``: Triggered when mount metadata/options change.

    If omitted, all three are monitored.

``args``
^^^^^^^^

    Arguments specific to ``mountnotify``:

    - ``mountpoints`` (**required**): list of mountpoint paths to watch.
    - ``locked`` (optional): if ``true``, the same event is sent only once and then muted.
      It will be sent again only after your event handler explicitly releases/unlocks it.

``tag``
^^^^^^^

    An optional tag to associate with the event. If specified, the event name will include this tag,
    allowing for easier identification and filtering of events.

    Event ID format:

    .. code-block:: text

        <sensor-id>|mountnotify[@tag]|<action>@<target>|0

Example
-------

Here is an example of how to monitor ``/`` and ``/mnt/data`` mount changes:

.. code-block:: yaml

    mounts:
        description: Track critical mountpoint changes
        listener: mountnotify
        opts:
            - mounted
            - unmounted
            - changed
        args:
            mountpoints:
                - /
                - /mnt/data
            locked: true
        tag: storage-watch
