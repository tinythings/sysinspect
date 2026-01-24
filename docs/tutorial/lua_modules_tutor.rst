.. raw:: html

   <style type="text/css">
     span.underlined {
       text-decoration: underline;
     }
     span.bolditalic {
       font-weight: bold;
       font-style: italic;
     }
   </style>

.. role:: u
   :class: underlined

.. role:: bi
   :class: bolditalic

.. _lua_tutorial:

Using Lua Modules
=================

.. note::
  This tutorial shows how to run Lua code via Sysinspect by installing a Lua runtime module and
  publishing Lua scripts as modules. It assumes you have a working Sysinspect installation and basic
  familiarity with Sysinspect concepts such as modules, models, actions, and entities.

Prerequisites
-----------------

Before you begin, ensure you have the following prerequisites:

- A working installation of Sysinspect.
- Basic Lua knowledge (functions, modules, returning tables).
- Access to the SysMaster node (the node hosting the package repository).
- Permission to register modules and to sync the cluster.

Installing Lua Runtime
----------------------

Sysinspect runs scripts through *runtime modules*. A runtime module is a normal Sysinspect module
that ships an interpreter (Lua in this case) plus any runtime glue needed to execute your scripts.

To run Lua scripts, Sysinspect must have a Lua runtime module available in the repository.

Build the runtime
^^^^^^^^^^^^^^^^^

If you build Sysinspect from source, build the project from the top-level directory:

.. code-block:: bash

  make

After a successful build, the Lua runtime binary is typically located here:

.. code-block:: bash

  $SRC/target/release/runtime/lua-runtime

(Use ``target/debug/...`` if you built a debug configuration.)

Register the runtime
^^^^^^^^^^^^^^^^^^^^

Register the runtime on the SysMaster so it becomes available in the module repository.
Replace the path with the actual path to your built binary:

.. code-block:: bash

  sysinspect module -A \
    --path /path/to/your/target/release/runtime/lua-runtime \
    --name "runtime.lua-runtime" \
    --descr "Lua runtime"

What this does:

1. ``--path`` points to the runtime binary you built.
2. ``--name`` is the module name you will reference from models.
3. ``--descr`` is a human-readable description (optional).

This adds the runtime into SysMaster's package repository. You can then verify the module is registered
by listing all available modules:

.. code-block:: bash

  sysinspect module -L

After registering the module, the cluster **needs to be synced** so all nodes receive the new module metadata:

.. code-block:: bash

    sysinspect --sync

Once the runtime is registered and the cluster is synced, you can publish Lua scripts that will run
through that runtime.


Installing Lua Modules
----------------------

Sysinspect can ship plain script files as modules. For Lua, the current packaging style is:

* You upload a directory tree (usually named ``lib``).
* Sysinspect preserves the directory structure.
* Those scripts are treated as a library attached to the Lua runtime.

Directory layout
^^^^^^^^^^^^^^^^

Assume your local working directory contains a ``lib`` directory with Lua scripts under a
runtime-specific path, for example:

.. code-block:: text

   lib/
     runtime/
       lua54/
         hello.lua
         reader.lua
         caller.lua
         site-lua/
           mathx/
             init.lua
             extra.lua

The important part is: upload the *directory* (``./lib``), not individual files, so the runtime
sees the same module paths on all nodes.

To maintain your set of Lua scripts, you can create your own directory tree like the one above.
Make changes you need and then re-upload the entire ``./lib`` directory to update the scripts
in the repository. It will overwrite existing scripts with the same paths.


Publish the scripts
^^^^^^^^^^^^^^^^^^^

Example scripts are provided in the ``modules/runtime/lua-runtime/examples`` directory of the Sysinspect
source tree. You can freely modify or extend these scripts for your own use. Assuming you navigated to that
directory, mentioned above, and have a ``./lib`` directory, publish the scripts like this:

1. Add the library tree to the repository (run this from the directory that contains ``./lib``):

   .. code-block:: bash

      sysinspect module -A --path ./lib -l

   The ``-l`` flag tells Sysinspect you are adding a library directory.

   .. important::

        Take a look into the structure of the ``./lib`` directory before uploading.
        All runtime modules are typically going to ``runtime/<ID>`` subdirectories.
        All dependencies are typically under ``site-<id>`` subdirectories.

        Always upload the *directory* (``./lib``), not individual files, so the runtime
        sees the same module paths on all nodes.

2. Sync the cluster so all nodes receive the new module metadata:

   .. code-block:: bash

      sysinspect --sync

3. Verify that the scripts are visible:

   .. code-block:: bash

      sysinspect module -Ll

   Example output (yours might be different):

   .. code-block:: text

      Type    Name                                    OS   Arch    SHA256
      ──────────────────────────────────────────────────────────────────────────
      script  runtime/lua54/caller.lua                Any  noarch  7aff...d8c5
      script  runtime/lua54/hello.lua                 Any  noarch  22ce...f2e1
      script  runtime/lua54/reader.lua                Any  noarch  8ce3...0135
      script  runtime/lua54/site-lua/mathx/extra.lua  Any  noarch  92ce...79e3
      script  runtime/lua54/site-lua/mathx/init.lua   Any  noarch  f636...f314

Once you've done this, the Lua scripts are available to be called from models.
There are three modules in the example set:
- ``hello.lua``: A demonstration of using packages and modules (``site-lua/mathx`` in this case).
- ``reader.lua``: Reads the ``VERSION`` field from ``/etc/os-release`` and returns it.
- ``caller.lua``: Demonstrates calling an external program from Lua and capturing its output.

This set of examples although minimal, is more than enough to produce simple powerful
configuration management modules, capable of being extended for any use case.

Calling a Lua module from a model
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

In current implementation, you :bi:`do not call Lua modules directly` from actions. To execute these Lua
modules, you reference the runtime module (``runtime.lua-runtime``) from an action. Your action passes
arguments telling the runtime which Lua module to invoke.

Example model snippet:

.. code-block:: yaml

   entities:
     - foo

   actions:
     my-example:
       descr: Read VERSION from /etc/os-release via Lua
       module: runtime.lua-runtime
       bind:
         - foo
       state:
         $:
           opts:
             args:
               rt.mod: reader

The confusing part here is the ``rt.mod`` argument under ``opts.args``. All arguments with the ``rt.*``
prefix are special runtime arguments that the Lua runtime module understands and are **not** passed to
the Lua script. Any other arguments (without the ``rt.*`` prefix) are directly passed to the Lua script
as normal arguments.

.. important::

    The ``rt.*`` arguments are runtime-specific. Different runtimes may have different
    ``rt.*`` arguments. To know them, run the embedded runtime manpage command. In this case,
    if you have a standard installation, run this command on the **SysMinion** node:

    .. code-block:: bash

       /usr/lib/sysinspect/runtime/lua-runtime --man

In this example, ``mod: reader`` means you would run the Lua module implemented by ``reader.lua``.
What *exactly* the Lua runtime expects for module naming depends on the runtime implementation,
but the intent is: keep your script name stable and call it by module name.

Run the action
^^^^^^^^^^^^^^

Execute the action against a node (minion) like this:

.. code-block:: bash

   sysinspect yourmodel/foo yourminion

Where:

* ``yourmodel`` is the model directory/name.
* ``foo`` is the bound entity instance.
* ``yourminion`` is the hostname.

To target all nodes, you can use ``*``. Once executed, you can also check the action result
by invokindg the Sysinspect terminal UI, used for merely checking if the results came back correctly:

.. code-block:: bash

   sysinspect --ui

Troubleshooting
^^^^^^^^^^^^^^^

* If the runtime is missing, confirm ``runtime.lua-runtime`` appears in ``sysinspect module -L``.
* If scripts are missing, confirm you uploaded ``./lib`` (the directory) and re-ran
  ``sysinspect --sync``.
* If module imports fail, verify your ``lib/runtime/lua54/...`` layout matches what the runtime
  expects, and that any ``site-lua`` modules are located under the same tree.
