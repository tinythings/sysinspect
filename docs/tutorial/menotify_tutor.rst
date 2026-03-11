.. _menotify_tutorial:

Using MeNotify Sensors
======================

.. note::

    This tutorial shows how to publish and use a Lua-based ``menotify`` sensor.
    It uses the shipped GitHub issues demo because it is a real networked
    example and does not require building a custom Rust listener.

Overview
--------

``menotify`` is a scripted sensor family. Instead of writing a new Rust sensor
for each user-space integration, you publish a Lua script into the shared
library tree and reference it with:

.. code-block:: yaml

    listener: menotify.<module>

For this tutorial, the module is:

.. code-block:: yaml

    listener: menotify.githubissues

The shipped demo polls issues on a public GitHub repository and emits a normal
Sysinspect event every time it sees a newly opened issue.

What you need
-------------

- A working Sysinspect installation.
- Access to the SysMaster node.
- A public GitHub repository you control.
- Permission to publish libraries and sync the cluster.

Files used in this tutorial
---------------------------

The example already exists in the source tree:

.. code-block:: text

    examples/demos/menotify/
      README.md
      sensors.cfg
      model.cfg
      lib/
        sensors/
          lua54/
            githubissues.lua

Only ``sensors.cfg`` and the ``lib`` tree matter for the actual sensor.

Step 1: Publish the Lua sensor
------------------------------

From the demo directory, publish the shared library tree:

.. code-block:: bash

    cd examples/demos/menotify
    sysinspect module -A --path ./lib -l

This uploads:

.. code-block:: text

    lib/sensors/lua54/githubissues.lua

That path is where ``menotify.githubissues`` is resolved on the minion.

Step 2: Install the sensor configuration
----------------------------------------

Copy ``sensors.cfg`` into the master's sensors tree, for example:

.. code-block:: text

    $MASTER/data/sensors/menotify/sensors.cfg

Then edit the demo sensor arguments:

.. code-block:: yaml

    sensors:
      github-public-issues:
        listener: menotify.githubissues
        args:
          owner: your-github-user-or-org
          repo: your-public-repo
          state: open
          per_page: 20
          user_agent: sysinspect-menotify-demo

At minimum, set:

- ``owner``
- ``repo``

Optional arguments already used by the script are:

- ``state``
- ``per_page``
- ``user_agent``
- ``token``
- ``api``
- ``bootstrap_emit_existing``

Step 3: Export and sync
-----------------------

Ensure the master exports that sensor scope:

.. code-block:: yaml

    config:
      master:
        fileserver.sensors:
          - menotify

Then sync the cluster:

.. code-block:: bash

    sysinspect --sync

After sensor configuration changes, restart the minion so the listener is
reloaded.

Step 4: Observe the sensor
--------------------------

Once the minion starts:

1. The first successful poll seeds the local in-memory cursor.
2. It emits nothing on that first pass.
3. Create a new issue in the configured GitHub repository.
4. On the next poll, the sensor logs something like:

   .. code-block:: text

      New issue here: #42 Example issue title

5. The sensor emits a normal Sysinspect event.

The demo routes those events to ``console-logger``, so the payload appears in
the minion log stream.

What the script does
--------------------

The demo script uses exactly the current v1 ``menotify`` APIs:

- ``ctx.args`` for configuration
- ``ctx.state`` for the last seen issue number
- ``http.get(...)`` for GitHub polling
- ``log.info(...)`` and ``log.error(...)`` for logging
- ``ctx.emit(...)`` for event creation

The emitted event uses:

.. code-block:: lua

    ctx.emit(data, {
        action = "opened",
        key = tostring(number),
    })

So the resulting event ID looks like:

.. code-block:: text

    github-public-issues|menotify.githubissues|opened@42|0

Where to go next
----------------

Once this example works, the next step is usually to replace the GitHub polling
logic with your own integration and keep the same packaging pattern:

1. Put your Lua script under ``lib/sensors/lua54/``.
2. Publish it with ``sysinspect module -A --path ./lib -l``.
3. Reference it as ``listener: menotify.<module>`` in ``sensors.cfg``.
4. Sync the cluster and restart the minion.
