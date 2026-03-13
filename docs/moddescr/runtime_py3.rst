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
* executes a selected script by name,
* passes the SysInspect request object into ``run(req)``,
* optionally forwards script logs back to SysInspect via ``rt.logs``,
* resolves shared Python libraries from the runtime ``site-packages`` namespace.

In normal model DSL, Python runtime modules are called through the virtual
``py3.<module>`` namespace. For example, ``module: py3.hello`` dispatches to
the installed ``runtime.py3`` runtime module and selects ``hello`` as the
runtime module name internally.

Script lookup and naming
~~~~~~~~~~~~~~~~~~~~~~~~

When you use a module name such as ``py3.hello`` in the model DSL, SysInspect
selects the installed ``runtime.py3`` dispatcher and resolves the suffix
(``hello`` in this example) into a Python file path under the runtime scripts
directory.

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
* ``config`` for the full runtime configuration payload,
* ``opts`` for options,
* ``ext`` for extra passthrough payload,
* ``host`` for descriptive host data.

At the runtime input boundary, the historical ``arguments`` / ``options``
shape is still accepted as an alias for ``args`` / ``opts``.

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

Portable helpers
^^^^^^^^^^^^^^^^

The Python runtime preinstalls a portable ``host`` helper object that reads
from ``req["host"]`` and exposes the same helper meanings as Lua and Wasm
guest helper code.

Available helper methods:

* ``host.trait(name)``
* ``host.has(name)``
* ``host.paths()``
* ``host.path(name)``

Use ``host`` for passive descriptive data. The source of truth remains
``req["host"]``, especially ``req["host"]["traits"]``.

Platform-specific helpers
^^^^^^^^^^^^^^^^^^^^^^^^^

The runtime also preinstalls a ``packagekit`` helper namespace for Python
modules:

* ``packagekit.available()``
* ``packagekit.status()``
* ``packagekit.history(names, count=10)``
* ``packagekit.packages()``
* ``packagekit.install(names)``
* ``packagekit.remove(names)``
* ``packagekit.upgrade(names)``

``packagekit`` is Linux-only and optional. It is an active helper namespace,
not part of the portable core contract. On systems without PackageKit,
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

Migration note
--------------

Python is no longer embedded in the SysInspect core. Direct native ``.py``
modules are not resolved by ``libsysinspect`` anymore.

Use the virtual ``py3.<module>`` namespace instead. Useful host data now comes
from the shared request payload and portable helpers:

* ``req["host"]["traits"]``
* ``req["host"]["paths"]``
* ``req["config"]``
* ``host.trait(...)`` / ``host.path(...)`` when helper sugar is more convenient
