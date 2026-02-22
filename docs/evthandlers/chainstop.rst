**Chain Stop**: *[corner case]* Break Cirquit Event Loop
====================================================================

.. note::

    This document explains how to use the **chainstop** event handler.

.. warning::

    The **chainstop** handler is a *special case* and should be used with caution. It is designed to break a chain of actions
    in specific scenarios, and improper use can lead to unintended consequences in your event processing.

    **Under normal operations, your model should be always idempotent!**

    This means that you should be able to run the same action multiple times without causing unintended side effects.
    If you find yourself needing to use the **chainstop** handler, it may indicate that your model is not properly
    designed for idempotency, and you should consider revising your model to ensure that it can handle repeated actions gracefully.

    *Or you are in a corner case (congratulations) — which is very typical in embedded world...*

Overview
--------

The *chainstop* handler lets you break a circuit event loop.

The Problem
^^^^^^^^^^^

In Sysinspect "events" are whatever your OS is doing: creating/deleting files, network activity, hardware interrupts, etc.
Sysinspect is listening to all that "stuff". So if something happens on the OS, a sensor is triggered (if configured) and
then an event is generated.

So far, so goood.

And then an event reactor, seeing that event, is calling something. If that "something" is an action or (worse) directly
a script handler, e.g. "pipescript" — a shortcut to avoid making things right, then we might have a problem: if the script
is not idempotent, it will cause an infinite loop.

Here is an example of such a loop:

1. Sensor is watching ``/etc/ssh/sshd_config`` file for changes.
2. Someone is changed that file.
3. Sensor detects the change and generates an event.
4. Event reactor reacts to that event and calls a script handler, e.g. "pipescript", which runs a script that blows that
   file away, replacing it with a content it likes.
5. Sensor detects the change again and generates another event...

Here. You have it.

So a proper way to do this, is to let your action do only what is *needed to be done* and do **not** do anything else,
if a target satisfies the desired state. The downside of this: the action will be always executed **twice**: first time
to enforce the state, and the second time because of the first time.

But there might be a corner case. For example, you don't go from sensor straight to hammering the file, but you
reroute this event to a some model. However, if that model is complex, takes resources etc, you might not want to
re-run it again, so it will go one more time to make sure that the conditions are in desired state.

For this matter, you can use the **chainstop** handler. It will break the circuit and stop the event loop right away.

How It Works
------------

The **chainstop** mechanism prevents feedback loops by locking the initiating sensor. When a sensor detects a change and
triggers an action that modifies the monitored resource, it creates a new event. Without a lock, this generates another
event, causing an infinite loop. The solution: lock the sensor at the source.

Step 1: Lock the Sensor
^^^^^^^^^^^^^^^^^^^^^^^

Configure the sensor that initiates the chain with the ``locked`` flag:

.. code-block:: yaml
    :caption: Sensor lock configuration

    sensors:
        some-sensor-id:
            ...
            locked: true

This tells Sysinspect to emit the Event Identifier (EID) **only once** from this sensor until the lock is released. The
lock automatically persists through the entire action chain, suppressing duplicate events from the same source.

.. note::

    The lock is released automatically when the **chainstop** handler fires, or when the lock TTL (Time To Live)
    expires — default is 5 seconds. This prevents orphaned locks if something goes wrong in your action chain.

Step 2: Release the Lock with Chainstop
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

At the **end of your action chain**, configure a final event pattern that triggers the **chainstop** handler. This
event should match the terminal state of your workflow — when the system has settled and all modifications are
complete.

.. code-block:: yaml
    :caption: Chainstop configuration at chain exit

    events:
        the|last|event|pattern: # <-- Final event signalling chain completion
            handlers:
                - chainstop
            chainstop:
                eids:
                    - "some|id|pattern|here"
                    - "another|id|pattern|here"
                    - ...
                verbose: true # or false (default: false)

Practical Example
^^^^^^^^^^^^^^^^^

During your action's execution, the monitored resource may change multiple times, each generating a new event that you
want to ignore. By locking the originating sensor and releasing the lock with **chainstop** at the workflow's end, you
ensure the entire chain runs to completion without interference. The sensor remains locked until **chainstop** fires,
suppressing all intermediate side-events that would otherwise restart the cycle.

Options
-------

``eids``
^^^^^^^^^^^^

    A list if EID patterns to match for releasing the lock. This should correspond to the EIDs emitted by the sensor you want to.
    Careful, as chainstop may release many locks at once. For example:

    .. code-block:: yaml

        eids:
            - "some|id|pattern|here"
            - "another|id|pattern|here"

``verbose``
^^^^^^^^^^^^

    **Optional.** If you set this to true, the handler will log detailed messages. For example:

    .. code-block:: yaml
        :caption: Enable verbose logging

        verbose: true
