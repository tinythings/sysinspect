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
    sysinspect cluster --shutdown
    sysinspect cluster --online
    sysinspect cluster --hopstart
    sysinspect --unregister 30006546535e428aba0a0caa6712e225

``--sync`` instructs minions to refresh cluster artefacts and then report
their current traits back to the master.

The ``cluster`` subcommand owns cluster-wide lifecycle actions:

.. code-block:: bash

    sysinspect cluster --online
    sysinspect cluster --online '*'
    sysinspect cluster --online --hostnames=db01,192.168.2.50
    sysinspect cluster --online --id 30006546535e428aba0a0caa6712e225

    sysinspect cluster --shutdown
    sysinspect cluster --shutdown 'web*'
    sysinspect cluster --shutdown --hostnames=db01,192.168.2.50
    sysinspect cluster --shutdown --id 30006546535e428aba0a0caa6712e225

    sysinspect cluster --hopstart
    sysinspect cluster --hopstart 'edge*'
    sysinspect cluster --hopstart --hostnames=android01,192.168.2.99
    sysinspect cluster --hopstart --id 30006546535e428aba0a0caa6712e225

Selector rules:

* ``--id`` means a real minion id
* positional selectors and ``--hostnames`` are host-oriented selectors
* ``--hopstart`` only issues start attempts for selected minions that are both
  offline and stored as ``hopstart`` backend nodes in the master CMDB
* ``--hopstart`` does not print a summary table; it only confirms that
  hopstart was issued and the operator should watch the master log

Network Operations
------------------

The ``network`` subcommand groups transport and minion-presence
operations.

.. code-block:: bash

    sysinspect network --add --hostnames=db01,db02
    sysinspect network --add --hostnames=root@db01:/opt/sysinspect
    sysinspect network --add --list ./hosts.txt --user bo
    sysinspect network --remove --hostnames=db01
    sysinspect network --remove --force --hostnames=192.168.2.50
    sysinspect network --upgrade --hostnames=edge01,edge02
    sysinspect network --status
    sysinspect network --status --pending
    sysinspect network --status --idle 'db*'
    sysinspect network --rotate 'web*'
    sysinspect network --rotate --id 30006546535e428aba0a0caa6712e225
    sysinspect network --info --id 30006546535e428aba0a0caa6712e225
    sysinspect network --info db01.example.net

Supported operations:

* ``--add`` onboards one or more hosts and is now the preferred way to deploy
  a new minion
* ``--remove`` stops, unregisters, and removes a locally managed install
* ``--upgrade`` replaces a locally managed minion binary and restarts it
* ``--status`` prints managed transport state for the selected minions
* ``--rotate`` stages or dispatches transport key rotation for the selected minions
* ``--info`` prints detailed registry-backed minion information for exactly one minion

Add/remove/upgrade input:

* ``--hostnames`` / ``--names`` accepts comma-separated entries
* ``--list`` reads one entry per line and ignores blanks and ``#`` comments
* ``--user`` supplies the default SSH login user when an entry does not embed
  its own ``user@host`` prefix
* entry grammar is ``[user@]host[:destination]``
* destination is optional; when omitted, the current default managed root is
  the remote user's ``$HOME/sysinspect``

Important onboarding assumptions and limits:

* SSH access must already work before ``network --add`` can succeed
* the selected remote user must be able to reach the destination root
* if the destination is not writable directly, Sysinspect uses ``sudo`` when
  the probe says it is available
* the current tested minion artefact coverage is driven by the published
  repository builds; unsupported remote OS/architecture pairs fail before
  upload

Preferred new-host workflow:

1. Publish or refresh the current minion build in the repository.
2. Run ``sysinspect network --add ...``.
3. Wait for the host to become ``online`` in ``sysinspect cluster --online``.
4. If needed, verify traits with ``sysinspect network --info <host-or-id>``.

What ``network --add`` actually does:

1. probes the remote host over SSH
2. selects the matching published ``sysminion`` artefact by remote platform
3. runs remote setup and writes a managed ``.local`` marker
4. seeds registration trust by reading the master's public RSA key from disk
   and registering against that fingerprint
5. starts the minion in daemon mode
6. waits for secure bootstrap progress
7. waits for full master-side readiness: online, traits, transport, startup
   sync, and sensors sync
8. writes startup inventory / CMDB data on the master, including backend
   ``hopstart``

``network --remove`` behavior:

* normal remove expects to understand the managed ``.local`` marker first
* if the install is ours, it stops the minion with ``sysminion --stop``,
  unregisters it on the master, and removes the managed root
* ``--force`` still unregisters and forgets the minion on the master even when
  the host is broken, absent, or unreachable

``network --upgrade`` behavior:

* only touches installs marked as locally managed by ``.local``
* replaces the minion binary only
* restarts the daemon
* waits for secure bootstrap and normal readiness again

Supported selectors:

* ``--id`` targets one minion by System Id
* ``--query`` or trailing positional query targets minions by hostname glob
* ``--traits`` further narrows the target set by traits query
* if no query is provided, the default selector is ``*``

For ``--info``, broad selectors are rejected. Use either one hostname/FQDN or ``--id``.

Transport status filters:

* ``--all`` shows all selected minions; this is the default
* ``--pending`` shows only minions with a non-idle rotation state
* ``--idle`` shows only minions with an idle rotation state

Troubleshooting ``network --add``
---------------------------------

Common failure cases and what they usually mean:

* SSH failure: the host was never reachable or the selected user was wrong
* missing matching artefact: the module repository has no ``sysminion`` build
  for the remote platform
* registration key mismatch: the master still has stale trust for that minion
  identity
* stale live session: the master still thinks another copy of that minion is
  connected
* secure bootstrap failure: registration happened, but trusted transport did
  not become healthy

Safe rerun behavior:

* rerunning the same ``network --add`` against an already managed host reports
  ``already added``
* rerunning against a broken managed destination requires ``--force`` so the
  old remnants are removed first
* rerunning after a partial failure keeps the install root, but cleans staged
  uploads and tries to stop any partial minion runtime

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

Deployment Profiles
-------------------

Deployment profiles describe which modules and libraries a minion is allowed
to sync. Profiles are assigned to minions through the ``minion.profile``
static trait.

Profile definitions:

.. code-block:: bash

    sysinspect profile --new --name Toto
    sysinspect profile --delete --name Toto
    sysinspect profile --list
    sysinspect profile --list --name 'T*'
    sysinspect profile --show --name Toto

Assign selectors to a profile:

.. code-block:: bash

    sysinspect profile -A --name Toto --match 'runtime.lua,net.*'
    sysinspect profile -A --lib --name Toto --match 'runtime/lua/*.lua'
    sysinspect profile -R --name Toto --match 'net.*'

Assign or remove profiles on minions:

.. code-block:: bash

    sysinspect profile --tag 'Toto,Foo' --query 'web*'
    sysinspect profile --tag 'Toto' --id 30006546535e428aba0a0caa6712e225
    sysinspect profile --untag 'Foo' --traits 'system.hostname.fqdn:db01.example.net'

Notes:

* ``--name`` is an exact profile name for ``--new``, ``--delete``, ``--show``, ``-A``, and ``-R``
* ``--name`` is a glob pattern for ``--list``
* ``--match`` accepts comma-separated exact names or glob patterns
* ``-l`` / ``--lib`` switches selector operations and listing to library selectors
* ``--tag`` and ``--untag`` update ``minion.profile`` on the targeted minions
* profile names are case-sensitive Unix-like names
* each profile file carries its own canonical ``name`` field; the filename is only storage
* new profile files are written with lowercase filenames, but existing indexed filenames remain valid even if they are mixed-case or arbitrary

Profile Data Model
------------------

The master publishes a dedicated ``profiles.index`` next to ``mod.index``.
Each profile entry points to one profile file plus its checksum:

.. code-block:: yaml

    profiles:
      Toto:
        file: totobullshit.profile
        checksum: deadbeef

Each profile file carries the actual profile identity and the allowed artefact
selectors:

.. code-block:: yaml

    name: Toto
    modules:
      - runtime.lua
      - net.*
    libraries:
      - lib/runtime/lua/*.lua

The filename is only storage. The canonical profile identity is the
case-sensitive ``name`` field inside the file. Newly created profile files
are written with lowercase filenames, but already indexed filenames are
still honored as-is.

Sync Behavior
-------------

During minion sync:

1. ``mod.index`` is downloaded from the fileserver
2. ``profiles.index`` is downloaded from the fileserver
3. the minion resolves its effective profiles from ``minion.profile``
4. the selected profile files are refreshed into ``$SYSINSPECT/profiles``
5. profile selectors are merged by union + dedup
6. module and library sync is filtered by that merged selector set
7. integrity cleanup removes now-forbidden artefacts

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
