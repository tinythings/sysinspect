Welcome!
These are expanded examples to get more comfortable with Python runtime
modules for SysInspect.

To use these, compile and install Python runtime first. If you build the
whole workspace using "make", you will get
target/<debug|release>/runtime/py3-runtime binary. Then install it:

1. sysinspect module -A --path /to/your/target/release/runtime/py3-runtime --name runtime.py3 --descr "Python 3 runtime"

   This will put Python runtime into your package manager repository on
   SysMaster side.

2. Sync your cluster:

   sysinspect --sync


Now install these example runtime modules. Current implementation expects
runtime Python modules and their shared libraries to be installed as a
library payload for the runtime.

To install these into your environment, do the following (assuming you
are literally HERE in the current directory):

1. Important to add $PATH_TO_HERE/lib (or just ./lib). This way the
   entire structure inside "lib" will be preserved:

   sysinspect module -A --path ./lib -l

2. Sync the cluster:

   sysinspect --sync

3. Verify they landed correctly:

   sysinspect module -Ll

You should see something like this:

 Type    Name                                       OS   Arch    SHA256
 ─────────────────────────────────────────────────────────────────────────────
 script  runtime/python3/caller.py                  Any  noarch  ...
 script  runtime/python3/hello.py                   Any  noarch  ...
script  runtime/python3/reader.py                  Any  noarch  ...
script  runtime/python3/site-packages/mathx/__init__.py Any noarch ...


Python module contract
======================

Each runtime Python module should export:

1. `run(req)` as the main entrypoint
2. optional documentation as either:
   `doc = {...}`
   `def doc(): return {...}`

Important:

The documentation payload must be the documentation object itself, not
wrapped as `{"doc": ...}`.

For static metadata, prefer `doc = {...}`.
Use `doc()` only when you actually need to compute or assemble the
documentation dynamically.


Below are modules description:

hello.py
========

  1. Imports a helper package from runtime site-packages
  2. Returns a simple calculation
  3. Provides documentation as a plain `doc` object


caller.py
=========

  1. Executes `ls -lah` on a given directory
  2. Returns stdout either as raw text or a list of lines
  3. Provides documentation through `doc()`


reader.py
=========

  1. Reads `/etc/os-release`
  2. Extracts `VERSION`
  3. Forwards logs back to SysInspect runtime output


Call these from a model using namespaces such as ``py3.hello`` or
``py3.reader``.
