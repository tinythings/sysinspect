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

In normal model DSL, Lua runtime modules are called through the virtual
``lua.<module>`` namespace. For example, ``module: lua.reader`` dispatches to
the installed ``runtime.lua`` runtime module and selects ``reader`` as the
runtime module name internally.

Script lookup and naming
~~~~~~~~~~~~~~~~~~~~~~~~

When you use a module name such as ``lua.reader`` in the model DSL, SysInspect
selects the installed ``runtime.lua`` dispatcher and resolves the suffix
(``reader`` in this example) to a Lua file in the configured runtime scripts
directory.

The runtime treats the script as an entry point. Keep the script focused on one task and prefer importing
shared helpers from the dependency directory described below.

Directory layout (Lua 5.4)
~~~~~~~~~~~~~~~~~~~~~~~~~~

To keep module scripts and their dependencies predictable, install files into the following locations
under ``${SYSINSPECT_SHARELIB_ROOT}``:

1. **Main scripts (modules)**

   Install entry-point scripts into::

      lib/runtime/lua/

2. **Dependency libraries**

   Install Lua libraries required by your scripts into::

      lib/runtime/lua/site-lua/

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

``[ANY]`` (type: string)
  Additional keyword arguments forwarded to the executed script. These values are made available to the
  script as global variables.

Practical notes
---------------

* Keep argument names unambiguous, since they become globals in the script.
* Prefer passing strings and parsing them in Lua when you need structured values.
* Use ``rt.logs`` while developing scripts, then disable it if you want quieter operation.

Logging
~~~~~~~

The runtime preinstalls a global ``log`` table in Lua modules with these
methods:

* ``log.error(...)``
* ``log.warn(...)``
* ``log.info(...)``
* ``log.debug(...)``

When ``rt.logs`` is enabled, emitted log lines are returned in the runtime
response payload under ``__sysinspect-module-logs``.

Built-in Helper Namespaces
--------------------------

The Lua runtime preinstalls a ``packagekit`` helper namespace for runtime
scripts on Linux systems where PackageKit is available over D-Bus.

Available helper functions:

* ``packagekit.available()``
* ``packagekit.status()``
* ``packagekit.history(names, count?)``
* ``packagekit.packages()``
* ``packagekit.install(names)``
* ``packagekit.remove(names)``
* ``packagekit.upgrade(names)``

``packagekit`` is optional and Linux-only. If PackageKit is unavailable, then
``packagekit.available()`` returns ``false`` and the other calls may raise a
runtime error when invoked.
