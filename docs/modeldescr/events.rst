Event Processing
================

.. note::

    This document explains how to define, configure and handle emitted events.

Overview
--------

SysInspect is event-driven system.

Events are emitted during the cycle check-up. Every time SysInspect calls an action
and the result arrives, an event occurs.

Events are used for absolutely everything: reports, feedback, integration etc.
One can use them to integrate SysInspect with any kind of Configuration Management
system (Ansible, Salt etc) or other tools.

Configuration
-------------

Event definition is pretty straight-forward: a handler is anchored to an event Id
whih contains corresponding handlers and their configuration.

Synopsis
^^^^^^^^

The following syntax is used to configure events definition:

.. code-block:: text
    :caption: Confguration Syntax

    events:
        <action id>/<bound entity>/<state>/<return code>:
            # List of attached handlers and their order
            handlers:
                - <list>

            # Specific handler configuration
            [handler-id]:
                <key>: <value>

``events``
^^^^^^^^^^

    This is the entire section of the Events configuration. It can be a part of ``model.cfg`` in the
    root, or it can be in its own subdirectory, e.g. ``/events/...``.

``<event id>``
^^^^^^^^^^^^^^

    An event Id is a path-like composite of three other Ids, such as:

    1. **Action Id**, which causes the event
    2. **Bound Entity Id**, which binds the action to the entity
    3. **State**, which calls default action settings or a specific one

    .. code-block:: text
        :caption: Even Id Format

        <action id>/<bound entity id>/<state>/<return code>

    The *event Id* is holding the rest of the behaviour for this event. Return code has the
    following syntax:

        - ``$`` — matches **any** return code, success (``0``) or any error code (non-zero).
        - ``0..255`` — event is processed only at the **specific** error code.
        - ``E`` — event is processed only at non-zero error code (error).

``handlers``
^^^^^^^^^^^^

    List of handlers that will be called in exactly that order as defined. Example:

    .. code-block:: yaml
        :caption: Handlers example

        handlers:
            - console-logger
            - system-log
            - run-program

    Handlers are typically a part of SysInspect and their full list is obtained
    from the command line passing ``--list-handlers``.

``<handler id>``
^^^^^^^^^^^^^^^^

    This is an actual ``key/value`` configuration container with arbitrary data for
    the specific handler. Its Id is the same as defined in ``handlers`` section.
    This section is completely optional in the event configuration for the handler.
    Some handlers might accept specific config what to do with this data, once
    event appears.

    .. code-block:: yaml
        :caption: Handler Config

        handlers:
            - some-foobar

        # Same Id as defined in "handlers" section
        some-foobar:
            key: value
            otherkey: othervalue

.. hint::

    As the events might be overwhelming, to easier manage them, the amount of event
    configuration files is unlimited and they can be stored in user-convenient sub-tree
    within the model.