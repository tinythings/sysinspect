.. _menotify_sensor_dev_tutorial:

Writing a MeNotify Sensor
=========================

.. note::

    This tutorial explains how to create a real ``menotify`` Lua sensor from
    scratch, package it, publish it, and run it on a minion. The goal is to
    show the practical shape of a sensor that polls something real, keeps
    local state, logs what it is doing, and emits normal Sysinspect events.

Why this exists
---------------

You do **not** want to write a new Rust sensor every time some third-party
system needs polling or a bit of glue logic.

That is exactly what ``menotify`` is for:

- write a Lua script
- put it under the sensor sharelib tree
- reference it as ``listener: menotify.<module>``
- sync the cluster
- restart the minion

No Sysinspect rebuild is needed for each new user-space integration.

What you are building
---------------------

In this tutorial, the final sensor will:

1. Poll an HTTP JSON endpoint.
2. Read configuration from ``args``.
3. Keep VM-local cursor state in ``ctx.state``.
4. Log useful information with ``log.info(...)``.
5. Emit one event per newly discovered item.

The concrete example is intentionally close to the shipped GitHub issues demo,
because that is a practical sensor shape:

- periodic polling
- authentication optional
- stateful dedup
- small JSON payloads

The listener will look like this:

.. code-block:: yaml

    listener: menotify.myissues

That means the Lua file must be named:

.. code-block:: text

    lib/sensors/lua/myissues.lua

Step 1: Create the library tree
-------------------------------

Create a local working directory with this layout:

.. code-block:: text

    my-menotify-sensor/
      lib/
        sensors/
          lua/
            myissues.lua

This is important:

- ``lib`` is the root you will publish
- ``sensors/lua`` is the current runtime root for ``menotify`` scripts
- the module name comes from the filename

If the file is called ``myissues.lua``, then the DSL listener is:

.. code-block:: yaml

    listener: menotify.myissues

Step 2: Write the script contract
---------------------------------

A ``menotify`` script must return a table with exactly one entrypoint:

- ``tick(ctx)``
- or ``loop(ctx)``

For most integrations, start with ``tick(ctx)``.

Minimal shape:

.. code-block:: lua

    return {
        tick = function(ctx)
        end
    }

Use ``tick(ctx)`` when:

- you are polling
- you want Rust to own the interval
- you do not need to block forever inside Lua

Use ``loop(ctx)`` only when the script itself must keep control and remain
inside one long-running execution flow.

Step 3: Know what `ctx` gives you
---------------------------------

The current v1 ``menotify`` API gives you:

Passive fields:

- ``ctx.id``
- ``ctx.listener``
- ``ctx.module``
- ``ctx.opts``
- ``ctx.args``
- ``ctx.interval``

Active helpers:

- ``ctx.emit(data, meta?)``
- ``ctx.sleep(seconds)``
- ``ctx.now()``
- ``ctx.timestamp()``
- ``ctx.state.get(key)``
- ``ctx.state.set(key, value)``
- ``ctx.state.has(key)``
- ``ctx.state.del(key)``

Global helpers:

- ``log.error(...)``
- ``log.warn(...)``
- ``log.info(...)``
- ``log.debug(...)``
- ``http.get(url, opts?)``
- ``http.request({...})``
- ``packagekit.available()``
- ``packagekit.status()``
- ``packagekit.history(names, count?)``
- ``packagekit.packages()``
- ``packagekit.install(names)``
- ``packagekit.remove(names)``
- ``packagekit.upgrade(names)``

Important rule:

- ``opts`` and ``args`` are yours
- Rust does not interpret them
- if you need a token, URL, threshold, or toggle, put it in ``args``

Linux-specific helper note:

- ``packagekit.*`` is available as an optional helper namespace
- it is intended for Linux scripts that want to poll PackageKit over D-Bus
- it is not part of the portable core contract
- using it is a script-level portability choice

Step 4: Start with configuration
--------------------------------

Suppose you want the sensor to poll:

- ``owner``
- ``repo``
- ``token`` optional
- ``per_page`` optional

That means your sensor configuration will later need:

.. code-block:: yaml

    sensors:
      github-public-issues:
        description: Poll issues on a public repository
        listener: menotify.myissues
        args:
          owner: your-github-user-or-org
          repo: your-public-repo
          state: open
          per_page: 20
          user_agent: sysinspect-menotify-demo

Inside Lua, you read those values with:

.. code-block:: lua

    local owner = ctx.args.owner
    local repo = ctx.args.repo

If your script cannot run without them, fail early and clearly:

.. code-block:: lua

    if ctx.args.owner == nil or ctx.args.repo == nil then
        log.error("myissues requires args.owner and args.repo")
        return
    end

Step 5: Build one HTTP request
------------------------------

``menotify`` currently gives you:

.. code-block:: lua

    local rsp = http.get(url, {
        headers = {
            ["Accept"] = "application/json"
        },
        parse_json = true,
        timeout = 30,
    })

The response object contains:

- ``rsp.status``
- ``rsp.ok``
- ``rsp.headers``
- ``rsp.body``
- ``rsp.json`` if JSON parsing was requested and succeeded

Always validate the response:

.. code-block:: lua

    if not rsp.ok then
        log.error("poll failed with HTTP status", rsp.status)
        return
    end

    if type(rsp.json) ~= "table" then
        log.error("expected JSON array/object response")
        return
    end

Do not assume the remote side behaves.

Step 6: Add state so the sensor is useful
-----------------------------------------

If a sensor polls repeatedly, it usually needs some kind of cursor:

- last seen id
- last timestamp
- last revision
- last checksum

For v1, ``ctx.state`` is VM-local in-memory state:

- it survives across ``tick()`` calls
- it is lost when the sensor restarts

That is enough for many polling sensors.

Typical pattern:

.. code-block:: lua

    local seeded = ctx.state.has("last_seen_number")
    local last_seen = tonumber(ctx.state.get("last_seen_number")) or 0

    if not seeded then
        ctx.state.set("last_seen_number", current_max)
        log.info("Seeded local cursor at", current_max)
        return
    end

This is the key point:

- first poll establishes baseline
- later polls emit only new items

That makes the sensor usable instead of noisy.

Step 7: Emit proper events
--------------------------

When your script finds something interesting, emit a normal Sysinspect event:

.. code-block:: lua

    ctx.emit({
        number = number,
        title = title,
        body = body,
        url = url,
    }, {
        action = "opened",
        key = tostring(number),
    })

The ``data`` object is your payload.

The ``meta`` object controls:

- ``action``
- ``key``

That produces an event ID like:

.. code-block:: text

    github-public-issues|menotify.myissues|opened@42|0

If you omit ``meta``:

- ``action`` becomes ``emitted``
- ``key`` becomes ``-``

In practice, it is usually worth setting both so routing stays readable.

Step 8: Log like an operator, not like a poet
---------------------------------------------

Use logs to describe operational facts:

- missing config
- poll failed
- cursor seeded
- new item found

Good:

.. code-block:: lua

    log.info("New issue here:", "#" .. tostring(number), issue.title or "")

Bad:

- huge log spam on every unchanged poll
- vague messages
- logs that repeat the full payload every time for no reason

The minion log should tell an operator what happened without drowning them.

Step 9: Full script shape
-------------------------

At this point, the whole sensor usually looks like:

1. validate ``args``
2. make HTTP request
3. validate response
4. compute cursor / dedup
5. emit one event per new item
6. update state

The shipped example already does exactly that:

.. code-block:: text

    examples/demos/menotify/lib/sensors/lua/githubissues.lua

Read it as the reference implementation for this tutorial.

Step 10: Publish the script
---------------------------

From the directory containing your local ``lib`` tree:

.. code-block:: bash

    sysinspect module -A --path ./lib -l

This publishes your sensor script as a normal shared library artefact.

Then sync the cluster:

.. code-block:: bash

    sysinspect --sync

Step 11: Add the sensor config
------------------------------

Install a ``sensors.cfg`` snippet on the master, for example:

.. code-block:: yaml

    sensors:
      github-public-issues:
        description: Poll issues on a public repository
        listener: menotify.myissues
        interval: 15
        args:
          owner: your-github-user-or-org
          repo: your-public-repo
          state: open
          per_page: 20
          user_agent: sysinspect-menotify-demo

    events:
      github-public-issues|menotify.myissues|opened@$|0:
        handlers:
          - console-logger

        console-logger:
          concise: false
          prefix: My Issues Sensor

This is enough to prove the sensor is alive:

- the event is emitted
- the event is routed
- the payload appears in logs

Step 12: Restart the minion
---------------------------

Sensors are long-running listeners. They are not reloaded magically in place.

After publishing the script and syncing sensor config:

- restart the minion

That gives the sensor a fresh Lua VM and reloads the script from disk.

Remember the current restart policy:

- sensor restart drops VM state
- script is reloaded
- in-memory ``ctx.state`` is lost

Step 13: Test the sensor properly
---------------------------------

Appendix: Polling PackageKit
----------------------------

For Linux-only integrations, ``menotify`` also exposes a small
``packagekit`` helper namespace.

Minimal shape:

.. code-block:: lua

    return {
        tick = function(ctx)
            if not packagekit.available() then
                log.warn("PackageKit is not available on this system")
                return
            end

            local st = packagekit.status()
            local hist = packagekit.history({ "bash", "openssl" }, 8)

            ctx.emit({
                locked = st.locked,
                daemon_state = st.daemon_state,
                transactions = st.transactions,
                history = hist,
            }, {
                action = "packagekit-poll",
                key = tostring(ctx.now()),
            })
        end
    }

This helper is intended for polling-friendly use-cases such as:

- checking whether the daemon is locked
- observing active transactions
- looking at recent history for selected packages

It is not a D-Bus signal watcher yet. It is a helper for scripted polling.

There are three separate things to test:

1. Lua logic
   - does it parse input correctly?
   - does it dedup correctly?
   - does it emit the right fields?

2. Packaging/layout
   - is the script under ``lib/sensors/lua/``?
   - does the listener name match the filename?

3. End-to-end runtime
   - does the minion load it?
   - does it poll?
   - does it emit events?

For the shipped GitHub issues example, the repository already contains an
integration test under:

.. code-block:: text

    examples/demos/menotify/githubissues_it.rs

That is a good model:

- keep core runtime tests in the crate
- keep example ownership with the example

Design advice
-------------

When writing a real ``menotify`` sensor, do:

- keep Rust generic
- keep integration-specific logic in Lua
- fail early on missing config
- avoid noisy logs
- emit small, structured payloads
- use ``ctx.state`` for cursors, not for large caches
- keep one event meaning one thing

No, don't do this:

- embed giant binaries in events
- abuse logs as data transport
- invent a second event model inside Lua
- assume persistence across restart

Where to look next
------------------

- Sensor reference: :doc:`../eventsensors/menotify`
- Practical usage walkthrough: :doc:`menotify_tutor`
- Working example:
  ``examples/demos/menotify/lib/sensors/lua/githubissues.lua``
