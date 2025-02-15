.. raw:: html

   <style type="text/css">
     span.underlined {
       text-decoration: underline;
     }
     span.bolditalic {
       font-weight: bold;
       font-style: italic;
     }
   </style>

.. role:: u
   :class: underlined

.. role:: bi
   :class: bolditalic

Actions
=======

.. note::
   This document describes entities definition

Actions are predefined batches of specific acts that are yielding
the state of an entity, based on its constraints. Actions are binding
data to modules.

.. important::

    The following rules are applied to an action:

    - An action is a consumer of claims of an entity
    - One action applies only to one claim, but it may statically refer claims from other entities

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
            opts|options:
              - <option>

            args|arguments:
              <key>:
                - <value>

.. important::

  Some sections have aliases.

  For better consistency, please use same group of aliases. For example, always use
  ``options/arguments`` across the entire model, or choose to use ``opts/args``.
  You can mix them, but it is **not the best practice**.

Below is the description of configuration sections:

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

    They are not. An entity anyway is checked through all defined states. If claims are matching for one particular state,
    it is concluded that the device is in this state. Another option is to pass an argument to a module of a state. So
    if a module is able to request a state, then it can match the return result accordingly.

    For example, if a router has two bands wifi state and one band wifi state, each action can send a flag, making
    the corresponding module aware of the currently processed state. Therefore, in case of the state is requested other
    than it is currently detected on the device, the module should return **true**.

``opts|options: [list]``

    Options element ``opts`` (or ``options``) specifies flags to the module, in case it is needed. For example, a module
    called ``sys.proc`` might have different modes, such as checking if a process at all runs
    and do nothing else, or return its PID or owner, even stop it, restart it etc — it depends on
    a module. In any case, options would be statically passed in this action. Example:

    .. code-block:: yaml

        opts:
          - info

    The example above is equivalent to a command line expression like this:

    ``some-program --info``

``args|arguments: key/[list]``

    The ``args`` (or ``arguments``) element specifies keywords to the module. One **distinct difference** from
    a classic keywords is that this is a ``key/[list]`` *(of values)* rather then a ``key/value``.
    Example:

    .. code-block:: yaml

        args:
          file:
            - /var/log/messages

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

    The *minimal* data structure of a claims must be identical!

In this example of two entities that have additional claims and one action that is checking
those processes. Module ``sys.proc`` receives a flag ``is-running`` which puts it to a
process checking mode, accepting ``process`` parameter of a currently processed claim.
In this case, ``sys.proc`` will accept ``/sbin/init`` and ``/usr/bin/syslogd`` file.

The claim ``discspace`` from ``my-special`` claim will be omitted.

.. code-block:: yaml

    entities:
      systemd:
        claims:
          my-claim:
            - default:
                path: /sbin/init
      syslogd:
        claims:
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
            opts:
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
    ``myclaim: "claim(foo),claim(bar)"``, however this is very discouraged practice and it is strongly
    recommended to change the module so it accepts a list of values instead of a comma-separated string.

Another example, showing static data references. Consider the following configuration:

.. code-block:: yaml

    entities:
    # An entity, describing a static configuration
      systemconf:
        descr: static system configuration
        claims:
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
        description: Validate syslogd claims

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
              - free-disk: "static(entities.syslogd.claims.storage.free)"
              - free-mem: "static(entities.systemconf.claims.mem.free)"

In the example above, function ``static(....)`` can statically reach any defined value of a claim.


Chain Conditions
----------------

Typically, and mostly for the configuration management, not all actions should fire one after another.
Sometimes it is needed to call an action :bi:`only if` something is ``true`` or ``false``.

The **Sysinspect** has a concept of "Chain Conditions". Unlike in other Configuration Management Systems,
where actions can require some other actions, **Sysinspect** is executing each statement in its precise
order. This restriction is on purpose: to avoid chaotic undebuggable mess, once your model grows really
big.

.. important::

  The restriction of executing each action in its order is by design on purpose: each required action
  just has to be placed prior to the action that requires them.

  It is that simple!

Action, however, has two flags that prevents it from running:

  ``if-true: <sibling-action>``
    In this case an Action will run only if a sibling action will **succeed**.

  ``if-false: <sibling-action>``
    Inverted to the ``if-true``, an Action will run only if a sibling action will **fail**.

In this example it is shown that the action ``delete-file`` will run only if ``create-file``
will succeed.

.. code-block:: yaml

    actions:
      create-file:
        ...

      delete-file:
        if-true: create-file

However, ``if-true`` can be only known if a corresponding constraint is defined to that action,
because the module itself does not define any kind of truth: it merely says if its state has been
changed or not. For example, the file can already exist there, made by someone prior, so it has
to be deleted. But we want to fire that action :bi:`if and only if` the file is really there.
We can run ``fs.file::info`` on it and get ``changed: true``. But that will then require more
coding and more constraints. We can, however, run ``fs.file::create`` and then have a constraint
that checks if the file is really there.

.. warning::

  Since actions can run in "blind mode" (no assertions), clauses ``if-[true|false]``
  require a valid constraints attached to the corresponding action!

Likewise chain conditions can be used for consistency check: if a specific device is working
as expected, no additional checks are needed (as an example).