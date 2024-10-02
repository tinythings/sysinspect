Actions
=======

.. note::
   This document describes entities definition

Actions are predefined batches of specific acts that are yielding
the state of an entity, based on its constraints. Actions are binding
data to modules.

.. important::

    The following rules are applied to an action:

    - An action is a consumer of facts of an entity
    - One action applies only to one fact, but it may statically refer facts from other entities

Synopsis
--------

Actions are descriptions of a specific workflow that binds a number of entities
with their data to a particular module. They are essentially a module callers,
that are targeting at least one entity or group of them.

A collection of actions requires its root block, started with ``actions``. Syntax
of an action as follows:

.. code-block:: text

    actions:
      <unique ID>
        module: <module namespace>
        bind:
          - <entity ID>

        state:
          $|<id>:
            options:
              - <option>

            args:
              <key>: <value>

``module: namespace``

    This element assigns the content of an action to a specific module that will process it.
    Example:

    .. code-block:: yaml

        module: sys.proc

``bind: [list]``

    This element binds entities to the action. I.e. an action will process every
    mentioned entity. Example:

    .. code-block:: yaml

        bind:
          - systemd
          - journald

``state : [map]``

    A configuration group for the particular state. It must be the same ID as state ID in the entities collection.
    If actions processing the system in a serial fashion without knowing what it is even discovered, then how exactly
    the state is determined?

    They are not. An entity anyway is checked through all defined states. If facts are matching for one particular state,
    it is concluded that the device is in this state. Another option is to pass an argument to a module of a state. So
    if a module is able to request a state, then it can match the return result accordingly.

    For example, if a router has two bands wifi state and one band wifi state, each action can send a flag, making
    the corresponding module aware of the currently processed state. Therefore, in case of the state is requested other
    than it is currently detected on the device, the module should return **true**.

``options: [list]``

    Options element specifies flags to the module, in case it is needed. For example, a module
    called ``sys.proc`` might have different modes, such as checking if a process at all runs
    and do nothing else, or return its PID or owner, even stop it, restart it etc — it depends on
    a module. In any case, options would be statically passed in this action. Example:

    .. code-block:: yaml

        options:
          - info

    The example above is equivalent to a command line expression like this:

    ``some-program --info``

``args: key/value``

    The ``args`` element specifies keywords to the module. Example:

    .. code-block:: yaml

        args:
          file: /var/log/messages

    The example above is equivalent to a command line expression like this:

    ``some-program --file=/var/log/messages``

    .. note::

        Arguments and options are not directly one-to-one transpose of a CLI arguments.
        They are just structures in JSON format, those still can be properly interpreted
        by a module.

    As per note above, if a specific program requires multiple same arguments, this still
    can be achieved by grouping them as a list under one argument. For example, if a CLI
    equivalent is needed to this:

    ``some-program --file=/var/log/messages --file=/var/log/dmesg``

    The form above still can be achieved in this form:

    .. code-block:: yaml

        args:
          file:
            - /var/log/messages
            - /var/log/dmesg

    In this case a module will get a JSON data with ``file`` key and a list of paths,
    that can be then translated by a module in whatever required format.


Examples of Actions
-------------------

Given there are entities, such as ``syslogd``, ``systemd`` etc, one can bind an action to these.

.. warning::

    The *minimal* data structure of a facts must be identical!

In this example of two entities that have additional facts and one action that is checking
those processes. Module ``sys.proc`` receives a flag ``is-running`` which puts it to a
process checking mode, accepting ``process`` parameter of a currently processed fact.
In this case, ``sys.proc`` will accept ``/sbin/init`` and ``/usr/bin/syslogd`` file.

The fact ``discspace`` from ``my-special`` fact will be omitted.

.. code-block:: yaml

    entities:
      systemd:
        facts:
          my-fact:
            - default:
                path: /sbin/init
      syslogd:
        facts:
          my-special:
            - default:
                path: /usr/bin/syslogd
                diskspace: 500Mb

    actions:
      verify-process-running:
        description: process is running
        module: sys.proc
        bind:
          - syslogd
          - systemd
        state:
          $:
            options:
              - is-running
            args:
              - process: "claim(path)"

In the example above, function ``claim(path)`` is the interpolated value. This is similar
to the Shell expression as such: ``$MY_VAR``.

.. note::

    It is deliberately a Limitation on interpolated templates to prevent "spaghetti code",
    keeping it all in declarative mode. Modules should be constructed the way they get
    a clear arguments without complex interpolations.

    In some rare cases one might create a comma-separated string, if that is very necessary:
    ``myfact: "claim(foo),claim(bar)"``, however this is very discouraged practice and it is strongly
    recommended to change the module so it accepts a list of values instead of a comma-separated string.

Another example, showing static data references. Consider the following configuration:

.. code-block:: yaml

    entities:
    # An entity, describing a static configuration
      systemconf:
        descr: static system configuration
        facts:
          default:
            - storage:
                type: SSD
                size: 2TB
                free: 500Mb
            - mem:
                free: 10Mb

    actions:
    # Same ID as end-entity
      syslogd-possible:
        # Description of the action that will be logged
        # The shorter, the better
        description: Validate syslogd facts

        # Path to the module namespace.
        # Modules are located in $module_root and namespace
        # is just a directory, where the last element is a module itself.
        # For example, "sys.info" is "$module_root/sys/info"
        #
        # Module key has more options.
        module: sys.info
        bind:
            - syslogd
        state:
          $:
            args:
              # Variable $(foo.bar) always refers to a full path from the document root.
              - free-disk: "static(entities.syslogd.facts.storage.free)"
              - free-mem: "static(entities.systemconf.facts.mem.free)"

In the example above, function ``static(....)`` can statically reach any defined value of a fact.
