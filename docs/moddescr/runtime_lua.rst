``runtime.lua``
===============

.. note::

    This document describes ``runtime.lua`` module usage.

Lua runtime
-----------

``runtime.lua`` is the SysInspect Lua runtime module. It enables you to execute Lua scripts as SysInspect
modules, which makes it convenient to extend inspections with small, self-contained checks written in Lua.

At a high level, the runtime:

* discovers Lua scripts in the configured scripts directory,
* executes a selected script (module) by name,
* optionally forwards keyword arguments to the script as global variables,
* can expose logging and native library loading depending on the configured options.

Script lookup and naming
~~~~~~~~~~~~~~~~~~~~~~~~

When you set the required keyword argument ``rt.mod``, SysInspect searches for a Lua file with that module
name in the predefined, configured scripts directory.

The runtime treats the script as an entry point. Keep the script focused on one task and prefer importing
shared helpers from the dependency directory described below.

Directory layout (Lua 5.4)
~~~~~~~~~~~~~~~~~~~~~~~~~~

To keep module scripts and their dependencies predictable, install files into the following locations
under ``${SYSINSPECT_SHARELIB_ROOT}``:

1. **Main scripts (modules)**

   Install entry-point scripts into::

      lib/runtime/lua54/

2. **Dependency libraries**

   Install Lua libraries required by your scripts into::

      lib/runtime/lua54/site-lua/

This separation keeps your callable modules easy to list and prevents helper libraries from being treated
as top-level SysInspect modules.

Options
-------

``rt.list``
  List available Lua scripts that can be called as modules.

``rt.logs``
  Enable logging from Lua scripts into SysInspect logs. Use this for diagnostics and traceability.

``rt.native``
  Enable loading of native Lua libraries (C modules). Use with caution, since native modules run with the
  same privileges as the SysInspect process and can widen the runtime's attack surface.

Keyword arguments
-----------------

``rt.mod`` (type: string, required)
  The name of the Lua script to execute. The runtime looks it up in the configured scripts directory.

``[ANY]`` (type: string)
  Additional keyword arguments forwarded to the executed script. These values are made available to the
  script as global variables.

Practical notes
---------------

* Keep argument names unambiguous, since they become globals in the script.
* Prefer passing strings and parsing them in Lua when you need structured values.
* Use ``rt.logs`` while developing scripts, then disable it if you want quieter operation.

