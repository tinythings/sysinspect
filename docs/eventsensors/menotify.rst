``menotify``: Scripted User-Space Sensors in Lua
================================================

The ``menotify`` sensor family loads a Lua script from the shared library tree
and runs it as a long-lived event sensor. This is meant for user-space and
third-party integrations where a built-in Rust listener would be too specific
or would require a rebuild for each new use-case.

The listener syntax is:

.. code-block:: yaml

    <id>:
        description: <description>
        listener: menotify.<module>
        [profile]:
          - <profile>
        [interval]: <interval override>
        [opts]:
          - <opaque option>
        [args]:
          key: <opaque value>
        [tag]: <optional tag>

Synopsis
--------

Sensor configuration as follows:

.. code-block:: yaml

    github-public-issues:
        description: Poll issues on a public GitHub repository
        listener: menotify.githubissues
        args:
            owner: your-github-user-or-org
            repo: your-public-repo
            state: open
            per_page: 20
            user_agent: sysinspect-menotify-demo

``listener``
^^^^^^^^^^^^

    The listener family root is always ``menotify``.

    The part after the dot is the Lua module name. For example:

    .. code-block:: yaml

        listener: menotify.githubissues

    loads:

    .. code-block:: text

        ${SYSINSPECT_SHARELIB_ROOT}/lib/sensors/lua54/githubissues.lua

``opts``
^^^^^^^^

    **Optional**

    This is a list of opaque options passed to the Lua script unchanged.
    Rust does not interpret them.

``args``
^^^^^^^^

    **Optional**

    This is a dictionary of opaque arguments passed to the Lua script
    unchanged. Rust does not interpret them.

    In current implementation, secrets are not a special API. If a scripted
    sensor needs a token or another secret, it is passed through ``args``.

``interval``
^^^^^^^^^^^^

    The effective polling interval used for ``tick(ctx)`` scripts.

    If the Lua script exports ``loop(ctx)``, it is called once and manages its
    own blocking behavior.

``tag``
^^^^^^^

    **Optional**

    If defined, the tag is included in the listener portion of the generated
    event ID.

    Example:

    .. code-block:: text

        github-public-issues|menotify.githubissues@demo|opened@42|0

Lua Contract
------------

The script must return a table with exactly one entrypoint:

.. code-block:: lua

    return {
        tick = function(ctx)
        end
    }

or:

.. code-block:: lua

    return {
        loop = function(ctx)
        end
    }

The current ``ctx`` fields and helpers are:

- ``ctx.id``
- ``ctx.listener``
- ``ctx.module``
- ``ctx.opts``
- ``ctx.args``
- ``ctx.interval``
- ``ctx.emit(data, meta?)``
- ``ctx.sleep(seconds)``
- ``ctx.now()``
- ``ctx.timestamp()``
- ``ctx.state.get(key)``
- ``ctx.state.set(key, value)``
- ``ctx.state.has(key)``
- ``ctx.state.del(key)``

Global Lua helpers:

- ``log.error(...)``
- ``log.warn(...)``
- ``log.info(...)``
- ``log.debug(...)``
- ``http.get(url, opts?)``
- ``http.request({...})``

Event Shape
-----------

``ctx.emit(data, meta?)`` builds a standard Sysinspect sensor envelope.

Example:

.. code-block:: lua

    ctx.emit({
        number = 42,
        title = "New issue"
    }, {
        action = "opened",
        key = "42"
    })

This becomes:

.. code-block:: text

    github-public-issues|menotify.githubissues|opened@42|0

``meta`` currently accepts:

- ``action``
- ``key``

If omitted:

- ``action`` defaults to ``emitted``
- ``key`` defaults to ``-``

Packaging and Sync
------------------

``menotify`` scripts are shipped as normal library/sharelib artefacts.

Current layout:

.. code-block:: text

    lib/
      sensors/
        lua54/
          <module>.lua
          site-lua/
            ...

Publish them with the standard library upload command:

.. code-block:: bash

    sysinspect module -A --path ./lib -l

Then sync the cluster:

.. code-block:: bash

    sysinspect --sync

Example
-------

The repository ships a working demo:

.. code-block:: text

    examples/demos/menotify/

It contains:

- ``lib/sensors/lua54/githubissues.lua``
- ``sensors.cfg``
- ``README.md``

That example polls GitHub issues on a public repository and emits one event per
newly opened issue.
