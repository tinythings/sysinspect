``runtime.py3``
===============

.. note::

    This document describes ``runtime.py3`` module usage.

Python runtime
--------------

``runtime.py3`` is the SysInspect Python 3.14 runtime module.
It enables you to execute Python scripts as SysInspect modules while keeping
the interpreter and its shared libraries inside the SysInspect runtime layout.

At a high level, the runtime:

* discovers Python scripts in the configured runtime directory,
* executes a selected script by name via ``rt.mod``,
* passes the SysInspect request object into ``run(req)``,
* optionally forwards script logs back to SysInspect via ``rt.logs``,
* resolves shared Python libraries from the runtime ``site-packages`` namespace.

Script lookup and naming
~~~~~~~~~~~~~~~~~~~~~~~~

When you set the required keyword argument ``rt.mod``, SysInspect resolves the
module name into a Python file path under the runtime scripts directory.

Examples:

* ``hello`` resolves to ``hello.py``
* ``nested.reader`` resolves to ``nested/reader.py``

Keep entry-point modules focused on one task and place reusable helpers in the
runtime ``site-packages`` directory described below.

Directory layout (Python 3)
~~~~~~~~~~~~~~~~~~~~~~~~~~~

To keep entry-point scripts and shared libraries predictable, install files
under ``${SYSINSPECT_SHARELIB_ROOT}`` in the following locations:

1. **Main scripts (modules)**

   Install entry-point Python modules into::

      lib/runtime/python3/

2. **Dependency libraries**

   Install reusable Python packages into::

      lib/runtime/python3/site-packages/

This separation keeps callable modules easy to list and prevents helper
packages from being treated as top-level SysInspect modules.

Python module contract
~~~~~~~~~~~~~~~~~~~~~~

Each runtime Python module is expected to export:

* ``run(req)`` as the main entrypoint,
* optional module documentation in one of these forms:

  * ``doc = {...}``
  * ``def doc(): return {...}``

In both cases the documentation payload must be the documentation object
itself, not wrapped as ``{"doc": ...}``.

For static metadata, prefer ``doc = {...}``.
Use ``doc()`` only when you actually need to assemble the documentation
dynamically.

The ``req`` object passed to ``run(req)`` contains the same main sections
used by other runtimes:

* ``args`` for keyword arguments,
* ``config`` for selected runtime configuration,
* ``opts`` for options,
* ``ext`` for extra passthrough payload.

Logging
~~~~~~~

The runtime preinstalls a ``log`` object in Python modules with these methods:

* ``log.error(...)``
* ``log.warn(...)``
* ``log.info(...)``
* ``log.debug(...)``

When ``rt.logs`` is enabled, emitted log lines are returned in the runtime
response payload under ``__sysinspect-module-logs``.

Helpers
~~~~~~~

The runtime also preinstalls a ``packagekit`` helper namespace for Python
modules:

* ``packagekit.available()``
* ``packagekit.status()``
* ``packagekit.history(names, count=10)``
* ``packagekit.packages()``
* ``packagekit.install(names)``

``packagekit`` is Linux-only and optional. On systems without PackageKit,
``packagekit.available()`` returns ``False`` and the other calls may raise a
runtime error if used anyway.

Options
-------

``rt.list``
  List available Python scripts that can be called as modules.

``rt.logs``
  Enable logging from Python scripts into SysInspect logs. Use this while
  developing or when you need traceability from runtime modules.

Keyword arguments
-----------------

``rt.mod`` (type: string, required)
  The name of the Python script to execute. The runtime looks it up in the
  configured scripts directory and supports dotted names for nested modules.

``[ANY]`` (type: string)
  Additional keyword arguments forwarded inside ``req["args"]`` to the executed
  module.

Practical notes
---------------

* Keep return values JSON-serialisable, since runtime output is converted back
  into JSON.
* Prefer putting shared helpers into ``site-packages`` instead of importing
  one callable module from another.
* Use ``rt.logs`` while developing runtime scripts, then disable it if you want
  quieter operation.
