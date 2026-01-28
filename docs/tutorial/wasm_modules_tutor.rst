.. _wasm_tutorial:

Using Wasm Modules
===================

.. note::
  This tutorial shows how to run WebAssembly (Wasm) modules in Sysinspect and explains
  how the Wasm runtime behaves internally.

Reasoning
---------

WebAssembly in Sysinspect is **not about browsers**.

Here, Wasm is used as a *portable execution format*, comparable to a Java ``.jar``:
a single, architecture-independent artifact that can be distributed and executed
consistently across heterogeneous systems.

Traditional configuration management systems (Ansible, Salt, Puppet, etc.) primarily execute
scripts or, in some cases, native binaries directly on the target system. As soon as compiled binaries
are involved, this approach becomes fragile:

- CPU architecture differences
- libc and ABI mismatches
- platform-specific packaging
- manual dependency logistics
- inconsistent runtime behavior

Sysinspect already solves binary logistics by distributing architecture-specific packages
automatically. However, Wasm enables a different and often simpler model:

- one **noarch** artifact
- sandboxed execution
- explicit host interaction
- predictable behavior across platforms

In Sysinspect, a Wasm module behaves operationally like a script, even though it is
compiled. You build it once, publish it once, and run it everywhere.

.. hint::

    Surely, one can compile native binaries for each architecture and develop binary modules directly.

    Yes, but this quickly becomes tedious and complex, as your module must also take care
    of direct integration with Sysinspect itself. Wasm offers a more elegant solution and isolates all
    the integration bits away from the module logic, allowing you to focus on the core functionality.

Prerequisites
-------------

Before you begin, ensure you have:

- A working installation of Sysinspect.
- Basic understanding of WebAssembly and WASI.
- Access to the SysMaster node.
- Permission to register modules and sync the cluster.
- A compiler capable of targeting WASI (for example: `TinyGo`_, `Zig`_, `Grain`_, `Swift`_, `Rust`_, and many more).

.. _TinyGo: https://tinygo.org
.. _Zig: https://ziglang.org
.. _Grain: https://grain-lang.org
.. _Rust: https://www.rust-lang.org
.. _Swift: https://swift.org

Installing Wasm Runtime
-----------------------

Sysinspect executes Wasm modules through a *runtime module*.
A runtime module is a standard Sysinspect module that embeds a Wasm engine and defines
how Wasm code interacts with the host system.

To execute Wasm binaries, a Wasm runtime must be present in the package repository.

Build the runtime
^^^^^^^^^^^^^^^^^

If building Sysinspect from source:

.. code-block:: bash

  make

The Wasm runtime binary is typically located at:

.. code-block:: bash

  $SRC/target/release/runtime/wasm-runtime

Register the runtime
^^^^^^^^^^^^^^^^^^^^

Register the runtime on the SysMaster:

.. code-block:: bash

  sysinspect module -A \
    --path /path/to/target/release/runtime/wasm-runtime \
    --name "runtime.wasm-runtime" \
    --descr "Wasm runtime"

Verify registration:

.. code-block:: bash

  sysinspect module -L

Then sync the cluster:

.. code-block:: bash

  sysinspect --sync

Installing Wasm Modules
-----------------------

Wasm user modules are distributed as compiled ``.wasm`` files. From Sysinspect’s
perspective, these are **noarch executable artifacts** handled by the Wasm runtime.

Directory layout
^^^^^^^^^^^^^^^^

Wasm modules are installed as a library tree, similar to Lua scripts:

.. code-block:: text

   lib/
     runtime/
       wasm/
         hellodude.wasm
         caller.wasm
         reader.wasm

The important rule is consistency: upload the directory tree, not individual files,
so all nodes observe identical paths.

Publish the modules
^^^^^^^^^^^^^^^^^^^

Upload the directory containing the Wasm modules:

.. code-block:: bash

  sysinspect module -A --path ./lib -l

Then sync the cluster:

.. code-block:: bash

  sysinspect --sync

Verify:

.. code-block:: bash

  sysinspect module -Ll

Execution Model and JIT Compilation
-----------------------------------

When a Wasm module is executed **for the first time** on a node, the runtime performs
a one-time compilation step:

- The ``.wasm`` file is translated into a cached native representation
  (``.cwasm`` internally).
- The cached artifact is an **ELF relocatable object**, specific to the host architecture.
- Subsequent executions reuse the cached artifact and are significantly faster.

This is conceptually similar to Python’s ``.pyc`` or ``.pyo`` files.

.. important::

   Cached ``.cwasm`` artifacts are **not** tracked by the Sysinspect package manager, because they are:

    - architecture-dependent
    - node-local
    - runtime-managed

   They must never be uploaded or distributed. They appear automatically and are removed automatically on the next sync.
   Shipping a cached artifact to a different architecture will result in undefined behavior.

Language Support
----------------

Any language capable of compiling to **WASI** can theoretically be used.

In practice, Sysinspect focuses on languages that produce **small**, **fast**, and
**predictable** Wasm binaries. There are two primary recommendations:

1. **TinyGo** is the recommended choice if you want it "in 10 minutes", **Rust** otherwise. TinyGo offers:

   - Relatively small binaries
   - Predictable output
   - Excellent WASI support
   - One can learn Go in a few hours
   - "Tons" of ready to use libraries

  .. important::

    You can also use standard Go with ``GOOS=wasip1 GOARCH=wasm``, but TinyGo produces much smaller
    and faster binaries.

2. **Rust** is your primary choice if you want maximum performance and control, but don't want it "in 10 minutes". **Rust** offers:

   - **Smallest/fastest binaries**
   - Excellent WASI support
   - Very mature toolchain
   - Excellent performance
   - Memory safety
   - Rich ecosystem

  .. note::

    Although Rust makes many things "right", yet it has a way much steeper learning curve. Even if you've
    mastered it enough, the development speed is not necessarily faster than with Go.

    At last, Configuration Management does not require systems programming skills and usually any
    CM module code is typically a "glue boilerplate", that can be done with higher-level languages.

  But it is still fun. :-)

C/C++ is also a solid choice, but you must take care of memory management and other low-level details yourself.
Other languages do *technically* work as well (Grain, Swift etc), but they aren't supported
in SysInspect realm. If you want to try them out, you should be prepared for one or more side effects:

   - Significantly larger binaries
   - Poor(-er) performance
   - Unstable toolchains
   - Randomly missing WASI features
   - Other bad surprises

While experimentation is encouraged, production modules should prioritise simplicity
and predictability. In any case, if you find a language that works well, please share your experience
with the community.

SDKs and Helper Libraries
-------------------------

Language-specific helper libraries and SDKs are expected to evolve as community
contributions.

At present, the Wasm runtime operates in **spartan mode**:

- minimal host API
- no language-specific abstractions
- explicit behavior over convenience

This reduces maintenance cost and keeps runtime behavior transparent.

Calling a Wasm module from a model
----------------------------------

Runtime-bound modules are not invoked directly at this moment. Instead, you reference the runtime module and
specify which submodule it should execute.

Example action:

.. code-block:: yaml

   call-hello:
     descr: Call WASM/WASI module
     module: runtime.wasm-runtime
     bind:
       - wasm
     state:
       $:
         args:
           rt.mod: hellodude
           key: PRIVACY_POLICY_URL

Here:

- ``runtime.wasm-runtime`` selects the runtime.
- ``rt.mod`` identifies the Wasm module.
- Arguments with the ``rt.*`` prefix are reserved for runtime configuration. You can always get runtime manual with
  directly calling the runtime module using ``--man`` argument.
- Arguments without the ``rt.*`` prefix are passed to the submodule "as is".

.. note::

   The exact syntax for runtime invocation may evolve in the future. A more
   unified namespace (for example ``runtime.wasm.<module>``) is planned, but
   requires additional module typing and namespace changes. For now, the
   explicit ``rt.mod`` approach is used.

Mixed Runtime Example
---------------------

A single model can freely combine Wasm, Lua, and native Sysinspect modules.

Example:

.. code-block:: yaml

   entities:
     - example

   actions:
     call-spawner:
       descr: Try spawner
       module: runtime.wasm-runtime
       bind: [example]
       state:
         $:
           args:
             rt.mod: caller

     get-os-version:
       descr: Return OS version
       module: runtime.lua-runtime
       bind: [example]
       state:
         $:
           opts: [rt.logs]
           args:
             rt.mod: reader

     ping:
       descr: Information module
       module: sys.run
       bind: [example]
       state:
         $:
           args:
             cmd: "cat /etc/machine-id"

This demonstrates that runtimes are **orthogonal**: each runtime handles its own
execution model, while Sysinspect orchestrates them uniformly when you call one ``example`` entity.

Troubleshooting
---------------

- Confirm the runtime appears in ``sysinspect module -L``.
- Ensure the Wasm module was compiled for WASI.
- Verify the library directory was uploaded and synced.
- Do not distribute cached runtime artifacts.
