Command Line Interface
======================

.. note::

    This section explains the CLI usage of all components.

Overview
--------

Sysinspect consists of three main executables:

1. ``sysinspect`` — the operator-facing command-line tool
2. ``sysmaster`` — the controller for connected minions
3. ``sysminion`` — the agent running on the target host

The rest of this page focuses on ``sysinspect`` itself.

Running Models Remotely
-----------------------

The most common use of ``sysinspect`` is sending a model query to the
master:

.. code-block:: bash

    sysinspect "my_model"
    sysinspect "my_model/my_entity"
    sysinspect "my_model/my_entity/my_state"

The optional second positional argument targets minions:

.. code-block:: bash

    sysinspect "my_model" "*"
    sysinspect "my_model" "web*"
    sysinspect "my_model" "db01,db02"

Use ``--traits`` to further narrow the target set:

.. code-block:: bash

    sysinspect "my_model" "*" --traits "system.os.name:Ubuntu"

Use ``--context`` to pass comma-separated key/value data into the model call:

.. code-block:: bash

    sysinspect "my_model" "*" --context "foo:123,name:Fred"

Running Models Locally
----------------------

``sysinspect`` can also execute a model locally without going through the
master. Use ``--model`` and optionally limit the selection by entities,
labels, and state:

.. code-block:: bash

    sysinspect --model ./my_model
    sysinspect --model ./my_model --entities foo,bar
    sysinspect --model ./my_model --labels os-check
    sysinspect --model ./my_model --state online

Cluster Commands
----------------

The following commands talk to the local master instance and affect the
cluster:

.. code-block:: bash

    sysinspect --sync
    sysinspect --online
    sysinspect --shutdown
    sysinspect --unregister 30006546535e428aba0a0caa6712e225

``--sync`` instructs minions to refresh cluster artefacts and then report
their current traits back to the master.

``--online`` currently prints the result into the master's log, because the
local control channel still has no response stream.

Traits Management
-----------------

Master-managed static traits can be updated from the command line:

.. code-block:: bash

    sysinspect traits --set "foo:bar"
    sysinspect traits --set "foo:bar,baz:qux" "web*"
    sysinspect traits --set "foo:bar" --id 30006546535e428aba0a0caa6712e225
    sysinspect traits --unset "foo,baz" "web*"
    sysinspect traits --reset --id 30006546535e428aba0a0caa6712e225

The ``traits`` subcommand supports:

* ``--set`` — comma-separated ``key:value`` pairs
* ``--unset`` — comma-separated keys
* ``--reset`` — clear only master-managed traits
* ``--id`` — target one minion by System Id
* ``--query`` or trailing positional query — target minions by hostname glob
* ``--traits`` — further narrow targeted minions by traits query

Module Repository Management
----------------------------

The ``module`` subcommand manages the master's module repository:

.. code-block:: bash

    sysinspect module -A --name runtime.lua --path ./target/debug/runtime/lua
    sysinspect module -A --path ./lib -l
    sysinspect module -L
    sysinspect module -Ll
    sysinspect module -R --name runtime.lua
    sysinspect module -R --name runtime/lua/reader.lua -l
    sysinspect module -i --name runtime.lua

Supported operations are:

* ``-A`` / ``--add``
* ``-R`` / ``--remove``
* ``-L`` / ``--list``
* ``-i`` / ``--info``

Use ``-l`` / ``--lib`` when operating on library payloads instead of runnable
modules.

TUI and Utility Commands
------------------------

``sysinspect`` also exposes a few utility entrypoints:

.. code-block:: bash

    sysinspect --ui
    sysinspect --list-handlers

The terminal user interface is documented separately in
:doc:`../uix/ui`.

Starting a Master
-----------------

To start a Master in foreground, issue ``--start`` option:

.. code-block:: text

    sysmaster --start

This option is also good for ``systemd`` as it runs in foreground, allowing the ``systemd``
or any similar init system taking over the service management.

However, in various use cases a standalone daemon option might be a preferred way to start
the Master. To start a Master as a standalone daemon, use ``--daemon`` option:

.. code-block:: text

    sysmaster --daemon

In this case, the ``sysmaster`` will be running as a daemon, remembering its PID. Stopping
it can be done either via SIGKILL -9 or ``--stop`` option.

Registering a Minion
--------------------

If one starts a minion for the first time, an error from the Master will be returned:

.. code-block:: text

    [15/12/2024 21:43:11] - ERROR: Minion is not registered
    [15/12/2024 21:43:11] - INFO: Master fingerprint: e79bc4ea44353c0933aacf5d84bd9e31063af8280325660a439946d7a4aee4a0

To register a minion, the following process should be performed:

1. Ensure that the Master's fingerprint is indeed as in the message above.
2. Copy the fingerprint of the Master.
3. Register the minion with ``--register`` option as follows:

.. code-block:: text

    ./sysminion  --register e79bc4ea44353c0933aacf5d84bd9e31063af8280325660a439946d7a4aee4a0

Output should be something like this:

.. code-block:: text

    [15/12/2024 21:47:03] - WARN: Preferred config at  does not exist, falling back
    [15/12/2024 21:47:03] - INFO: Initialising minion
    [15/12/2024 21:47:03] - INFO: Loading system traits data
    [15/12/2024 21:47:04] - INFO: Loading network traits data
    [15/12/2024 21:47:04] - INFO: Loading trait functions
    [15/12/2024 21:47:04] - INFO: Registration request to 10.10.2.75:4200
    [15/12/2024 21:47:04] - INFO: Minion registration has been accepted

Now the minion is ready to start.

Starting a Minion
-----------------

Operation of a Minion is identical to Master. To start a minion in foreground, simply use
``--start`` option:

.. code-block:: text

    sysminion  --start

If connection was established successfully, then the last message should be "Ehlo", something like this:

.. code-block:: text

    [15/12/2024 21:48:47] - INFO: Ehlo on 10.10.2.75:4200

To start/stop a Minion in daemon mode, use ``--daemon`` and ``--stop`` respectively.

Removing a Minion
-----------------

To remove a Minion (unregister) use the following command by its System Id:

.. code-block:: text

    sysinspect --unregister 30006546535e428aba0a0caa6712e225

In this case the Minion will be unregistered, its RSA public key will be removed, connection terminated
and the Master will be forgotten. In order to start this minion again, please refer to the Minion
registration.
