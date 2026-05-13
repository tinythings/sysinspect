``sys.pkg``
============

.. note::

    This document describes ``sys.pkg`` module usage.

Synopsis
--------

Cross-platform package management module. It inspects package state
(installed version, available version, upgradable), installs, removes,
updates, upgrades and searches packages across FreeBSD, OpenBSD, NetBSD,
Linux and macOS.

This module follows the Sysinspect inspect-then-act philosophy: operations
like ``install``, ``remove`` and ``upgrade`` first check the current state
and skip the action if it is already satisfied (e.g. skip install if the
package is already present). The ``force`` option bypasses this check.

Usage
-----

The following options are available:

  check
    Inspect package state. Returns structured data with ``installed``,
    ``installed_version``, ``available_version``, ``upgradable`` and
    ``repo`` fields. No changes are made to the system.

  install
    Install a package. The module first checks whether the package is
    already installed — if so, the operation is skipped with a status
    message. Use ``force`` to always run the install command.

  remove
    Remove a package. The module first checks whether the package is
    installed — if not, the operation is skipped. Use ``force`` to
    always run the remove command.

  update
    Update package repository metadata (e.g. ``pkg update`` or
    ``apt-get update``). No inspection is needed.

  upgrade
    Upgrade one or all packages. If a package name is given, the module
    checks whether it is actually upgradable before proceeding. For
    bulk upgrades (no name argument), the module lists upgradable
    packages first and reports how many will be upgraded.

  search
    Search for a package by name. No inspection is needed.

  dry-run
    Print the command that *would* be run, without executing it. Useful
    for testing and debugging across different operating systems.

  force
    Skip the pre-inspection step and execute the operation immediately,
    regardless of current package state.

The following keyword arguments are available:

  name (type: string)
    Package name. Required for ``check``, ``install``, ``remove`` and
    ``search``. Optional for ``update`` (ignored) and ``upgrade``.

Supported Package Managers
--------------------------

+------------------+----------------------------------------------+
| Platform         | Package Manager                              |
+==================+==============================================+
| FreeBSD          | ``pkg``                                      |
+------------------+----------------------------------------------+
| OpenBSD          | ``pkg_add`` / ``pkg_delete`` / ``pkg_info``  |
+------------------+----------------------------------------------+
| NetBSD           | ``pkgin`` (preferred) or ``pkg_add``         |
+------------------+----------------------------------------------+
| Linux            | ``apt-get``, ``dnf``, ``yum``, ``zypper``,   |
|                  | ``pacman`` or ``apk`` (auto-detected)        |
+------------------+----------------------------------------------+
| macOS            | ``brew`` (Homebrew)                          |
+------------------+----------------------------------------------+

Examples
--------

Check if a package is installed and whether an upgrade is available
(the ``$`` state key selects the default state):

.. code-block:: yaml

    actions:
      check-nginx:
        module: sys.pkg
        bind:
          - target-host
        state:
          $:
            opts:
              - check
            args:
              name:
                - nginx

Install a package (only if not already present):

.. code-block:: yaml

    actions:
      install-nginx:
        module: sys.pkg
        bind:
          - target-host
        state:
          $:
            opts:
              - install
            args:
              name:
                - nginx

Force reinstall even if the package is already there:

.. code-block:: yaml

    actions:
      force-install-nginx:
        module: sys.pkg
        bind:
          - target-host
        state:
          $:
            opts:
              - install
              - force
            args:
              name:
                - nginx

Dry-run an install to see what command would be executed:

.. code-block:: yaml

    actions:
      dry-run-install:
        module: sys.pkg
        bind:
          - target-host
        state:
          $:
            opts:
              - install
              - dry-run
            args:
              name:
                - nginx

Update repository metadata and upgrade all packages:

.. code-block:: yaml

    actions:
      refresh-packages:
        module: sys.pkg
        bind:
          - target-host
        state:
          $:
            opts:
              - update
              - upgrade

Returning Data
--------------

check
    Returns structured package state suitable for CM decision-making.

    .. code-block:: json

        {
          "data": {
            "name": "bash",
            "installed": true,
            "installed_version": "5.2.21",
            "available_version": "5.2.21",
            "upgradable": false,
            "repo": "main"
          },
          "retcode": 0,
          "message": "Package 'bash' is installed (v5.2.21) — up to date"
        }

install / remove / update / upgrade / search
    Default return for mutation operations.

    .. code-block:: json

        {
          "retcode": 0,
          "message": "Package operation 'install' completed"
        }
