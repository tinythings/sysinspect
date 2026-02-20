Event Sensors
==============

.. note::

    This document explains how to use **event sensors** and how Sysinspect routes events to handlers.

Overview
========

Every time in your system something happens. That "something" is an event. SysInspect can react to specific
events by listening to them in real time. This allows you to build powerful monitoring and automation solutions
that respond to changes in your system as they happen.

Sensors are defined separately from the models. While models are downloaded and refreshed on demand, every time
minion is calling them, sensors are always running in the background, listening for events right as Minion starts.
Sensor configuration cannot be changed on the fly just like that, because they are always running. Sensor updates
are applied on the next Minion restart or when you issue cluster sync command:

.. code-block:: bash

    sysinspect --sync

This is because sensors facility must stop listening to the whole system, sync configuration, pick a proper profile
and start again.

Sensors, just like models, are merged together from a different snippets. On a Master, they are placed in the ``sensors``
subdirectory. By default it is under ``/etc/sysinspect/data/sensors``. The rest of the directory structure is completely
up to you. They are merged together into a single configuration on the Master and then distributed to Minions.

.. important::

    If you accidentally define same IDs with the different configurations, the rule is "first wins" — the files are sorted
    alphabetically.


Synopsis
--------

There are few important things to keep in mind when working with sensors:

1. **Local Mode Behavior**: When sensors are defined in your configuration, the local version of Sysinspect
   will run continuously in the foreground. To run it as a background daemon, use the ``--daemon`` flag
   with the ``sysinspect`` command.

2. **Event Handler Responsibility**: It is the responsibility of the user to define what actions should be
   taken when events occur. Sensors themselves are purely passive listeners; they only emit the captured
   event data in JSON format to the event handler. No automatic actions are triggered by sensors alone.

3. **Separation of Concerns**: Sensors are independent components and not part of any data model. They serve
   as the listening configuration layer for a Sysinspect agent (whether SysMinion or local Sysinspect mode).
   Their sole purpose is to detect and report system events. The decision of what to do in response, which
   models to invoke, or whether to take any action at all, is entirely up to the user.

4. **Event Model Integration**: Event definitions are separated from models and moved into their own sensors
   configuration namespace. Once an event is emitted, it can be routed to any handler, which can then trigger
   models or other actions based on the event data.


Sensor Configuration Syntax
-----------------------------

The following shows the complete structure for configuring sensors. This YAML-based syntax defines how
sensors listen for events and emit notifications:

.. code-block:: text

    sensors:
      interval:
        max: int # default 10
        min: int # default 3
        unit: seconds|minutes|hours

      <id>:
        profile:
          - <profile1>
          - <profile2>
        description: <description of the sensor>
        listener: <listener type>
        interval: <interval in global units, optional, overrides global one>
        opts:
          - <option1>
          - <option2>
        args:
          key: <value>
        event: <event ID to emit on trigger, optional>

``profile``
^^^^^^^^^^^

    **Optional**

    This is the list of Minion profiles that matches the sensor. For example, if a minion has profile ``QNX``, then a sensor with
    the profile ``Linux`` won't be picked up. IF not defined, **all sensors** will be applied.

``description``
^^^^^^^^^^^^^^^

    A human-readable description of the sensor. This is purely informational and does not affect the sensor's behavior.

``listener``
^^^^^^^^^^^^

    This specifies the type of implemented supported listener that the sensor uses to detect events.
    Refer to the list of supported listeners in the documentation for more details on how to configure each listener type.

``interval``
^^^^^^^^^^^^

    Typically, all sensors are made on polling, which means they check for events at regular intervals. This is a pragmatic choice
    to provide a simple and consistent way to detect events across different systems and use cases. The ``interval`` field allows
    you to specify how often the sensor should check for events. You can set it globally for all sensors or override it for individual sensors.

    .. important::

        Most sensors are **using polling** under the hood, in order to provide a consistent and simple way to detect events across different
        systems and use cases.

    The interval can be set manually or calculated randomly between a minimum and maximum value. This is useful to avoid thundering
    herd problems when you have many sensors running at the same time.

``opts``
^^^^^^^^^^

    **Optional**

    This is a list of options specific to the listener type. Please refer to the documentation of a specific sensor.

``args``
^^^^^^^^^^

    **Optional**

    This is a dictionary of additional arguments that the listener might require. Please refer to the documentation
    of a specific sensor for details on what arguments are needed.

``event``
^^^^^^^^^^

    A sensor typically emits an event, constructing its own event path, based on the sensor specifics and implementation.
    However, in some corner cases it might be necessary to define (override) the event ID with a specific static value.

    Highly **not recommended** to use on a regular basis. But the feature is here if needed.

Available Sensors
=================

Here are the available sensors. This list is not exhaustive and may be updated as new sensors are added:

.. toctree::
  :maxdepth: 1

  fsnotify
  procnotify