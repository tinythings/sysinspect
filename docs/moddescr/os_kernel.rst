``os.kernel``
==============

.. note::

    This document describes ``os.kernel`` module usage.

Synopsis
--------

The ``os.kernel`` module manages kernel modules — the loadable pieces of
operating-system code that provide drivers, filesystems and other
kernel-level functionality. You can load them, unload them, check whether
they are already loaded, get detailed metadata about them, and list
everything currently attached to the kernel.

It works across FreeBSD, NetBSD, OpenBSD, Linux, macOS, Solaris and
Android by detecting whichever module tooling is available and speaking
its language. On OpenBSD — where the kernel is monolithic and has no
runtime loading — the module responds honestly: ``load`` and ``unload``
return an explanation, while ``status`` and ``info`` check the kernel
message buffer.

Like all Sysinspect modules, ``os.kernel`` is YAML-driven: all manager
knowledge is embedded at compile time, and a user-supplied
``os.kernel.yaml`` library file can override or extend it without
recompilation.

Usage
-----

The following options are available:

  ``status``
    Check whether a kernel module is loaded. Returns retcode 0 when
    loaded and 1 when not.

  ``load``
    Load a kernel module. Inspects first — if the module is already
    loaded the operation is skipped.

  ``unload``
    Unload a kernel module. Inspects first — if the module is not
    loaded the operation is skipped.

  ``info``
    Return detailed metadata about a kernel module: version, size,
    dependencies, filesystem path, license, author — whatever the
    platform can provide. Read-only, no changes to the system.

  ``list``
    List all currently loaded kernel modules with their size, reference
    count and other platform-specific fields.

  ``dry-run``
    Print the command that *would* be executed without running it.

The following keyword arguments are available:

  ``name`` (type: string)
    Kernel module name. Required for ``status``, ``load``, ``unload``
    and ``info``. Ignored by ``list``.

Supported Platforms
-------------------

+------------------+----------------------------------------------+
| Platform         | Tooling                                       |
+==================+==============================================+
| FreeBSD          | ``kldload`` / ``kldunload`` / ``kldstat``     |
+------------------+----------------------------------------------+
| NetBSD           | ``modload`` / ``modunload`` / ``modstat``     |
+------------------+----------------------------------------------+
| OpenBSD          | Monolithic kernel — ``dmesg`` for inspection  |
+------------------+----------------------------------------------+
| Linux            | ``modprobe`` / ``lsmod`` / ``modinfo``        |
+------------------+----------------------------------------------+
| macOS            | ``kextload`` / ``kextunload`` / ``kextstat``  |
+------------------+----------------------------------------------+
| Solaris/Illumos  | ``modload`` / ``modunload`` / ``modinfo``     |
+------------------+----------------------------------------------+
| Android          | ``insmod`` / ``rmmod`` / ``lsmod``            |
+------------------+----------------------------------------------+

Custom Managers
---------------

Override or extend the built-in manager list by placing an
``os.kernel.yaml`` file in the Sysinspect library directory
(``$SHARELIB/lib/os.kernel.yaml``):

.. code-block:: yaml

    managers:
      my-custom-kmod:
        os: linux
        detect: "test -x /opt/my-kmod/bin/ctl"
        description: "My custom kernel module manager"
        load: "/opt/my-kmod/bin/ctl load {name}"
        unload: "/opt/my-kmod/bin/ctl unload {name}"
        status: "/opt/my-kmod/bin/ctl status {name}"
        info: "/opt/my-kmod/bin/ctl info {name}"
        list_modules: "/opt/my-kmod/bin/ctl list"

Install it as a library:

.. code-block:: text

    sysinspect module -A -l --path /path/to/os.kernel.yaml

Examples
--------

Check if the pf firewall module is loaded:

.. code-block:: yaml

    actions:
      check-pf:
        module: os.kernel
        bind:
          - target-host
        state:
          $:
            opts:
              - status
            args:
              name:
                - pf

Get detailed information about the ZFS module:

.. code-block:: yaml

    actions:
      inspect-zfs:
        module: os.kernel
        bind:
          - target-host
        state:
          $:
            opts:
              - info
            args:
              name:
                - zfs

List all loaded kernel modules:

.. code-block:: yaml

    actions:
      list-modules:
        module: os.kernel
        bind:
          - target-host
        state:
          $:
            opts:
              - list

Load the fuse module (skipped if already loaded):

.. code-block:: yaml

    actions:
      enable-fuse:
        module: os.kernel
        bind:
          - target-host
        state:
          $:
            opts:
              - load
            args:
              name:
                - fuse

Returning Data
--------------

``status``
    Returns loaded state. retcode 0 when loaded, 1 when not.

    .. code-block:: json

        {
          "retcode": 0,
          "message": "Kernel module 'pf' is loaded",
          "data": {
            "name": "pf",
            "loaded": true,
            "manager": "freebsd-kld"
          }
        }

``info``
    Rich metadata. Fields vary by platform — Linux provides version,
    description, author, license, dependencies and path; others
    provide a subset.

    .. code-block:: json

        {
          "retcode": 0,
          "data": {
            "name": "zfs",
            "loaded": true,
            "manager": "linux-modprobe",
            "version": "2.2.0",
            "description": "ZFS filesystem",
            "license": "CDDL",
            "dependencies": ["spl"],
            "path": "/lib/modules/6.1.0/zfs/zfs.ko"
          }
        }

``list``
    Returns an array of loaded modules with platform-specific fields.

    .. code-block:: json

        {
          "retcode": 0,
          "data": {
            "manager": "linux-modprobe",
            "modules": [
              { "name": "zfs", "loaded": true, "size_bytes": 3670016 },
              { "name": "pf",   "loaded": true, "size_bytes": 123456 }
            ]
          }
        }

``load`` / ``unload``
    Mutation operations return the outcome and telemetry.

    .. code-block:: json

        {
          "retcode": 0,
          "message": "Kernel module 'fuse' load successful",
          "data": {
            "name": "fuse",
            "manager": "linux-modprobe",
            "exit_code": 0
          }
        }

Quick Test
----------

List all loaded kernel modules on the current machine:

.. code-block:: sh

    echo '{"options":["list"]}' | target/debug/kernel | jq '.data.modules[:3]'

    # Output:
    # [
    #   { "size_bytes": 110592, "name": "ufs",  "loaded": true },
    #   { "size_bytes": 53248,  "name": "isofs", "loaded": true },
    #   { "name": "uas", "size_bytes": 28672, "loaded": true }
    # ]

Inspect a specific module:

.. code-block:: sh

    echo '{"options":["info"],"arguments":{"name":"ufs"}}' | target/debug/kernel | jq .
