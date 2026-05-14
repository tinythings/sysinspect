``fs.dir``
==========

.. note::

    This document describes ``fs.dir`` module usage.

Synopsis
--------

The ``fs.dir`` module manages directory state through three idempotent
operations: inspect, ensure, and remove. It checks the current state
before acting — no unnecessary syscalls, no spurious ``changed`` flags.

The module is ``#![no_std]`` and makes no heap allocations, using only
raw libc calls. It reads a JSON request on stdin and writes a JSON
response to stdout.

Usage
-----

The following options are available (pick exactly one operation):

  ``check``
    Inspect a directory's existence, type, mode, owner, and group.
    Returns structured data suitable for CM decision-making. Read-only.
    Must be combined with argument ``name``.

  ``present``
    Ensure a directory exists with the given attributes. If it already
    exists, mode and ownership are updated if they differ. If mode, uid,
    and gid already match, the module reports no change.

  ``absent``
    Remove a directory if it exists. If the directory is not there, the
    module reports success with a descriptive message. Only removes empty
    directories (``rmdir`` semantics).

  ``dry-run``
    Can be combined with any operation. Prints what would be done without
    making changes. Does not modify the filesystem.

The following keyword arguments are available:

  ``name`` (type: string, required)
    Full path to the target directory.

  ``mode`` (type: string)
    Octal permission mode, e.g. ``0755``. Default is ``0755``.

  ``uid`` (type: int)
    Owner UID. Set to ``0`` to skip ownership changes. Default: ``0``.

  ``gid`` (type: int)
    Group GID. Set to ``0`` to skip group changes. Default: ``0``.

Examples
--------

Create ``/etc/myapp/config`` with default permissions:

.. code-block:: yaml

    actions:
      ensure-config-dir:
        module: fs.dir
        bind:
          - target-host
        state:
          $:
            opts:
              - present
            args:
              name:
                - /etc/myapp/config

Create a directory with specific mode and ownership:

.. code-block:: yaml

    actions:
      ensure-data-dir:
        module: fs.dir
        bind:
          - target-host
        state:
          $:
            opts:
              - present
            args:
              name:
                - /var/lib/myapp/data
              mode:
                - "0700"
              uid:
                - 1000
              gid:
                - 1000

Inspect a directory before deciding:

.. code-block:: yaml

    actions:
      check-log-dir:
        module: fs.dir
        bind:
          - target-host
        state:
          $:
            opts:
              - check
            args:
              name:
                - /var/log/myapp

Remove a stale cache directory:

.. code-block:: yaml

    actions:
      purge-cache:
        module: fs.dir
        bind:
          - target-host
        state:
          $:
            opts:
              - absent
            args:
              name:
                - /tmp/stale-cache

Dry-run to see what would happen:

.. code-block:: yaml

    actions:
      preview-dir:
        module: fs.dir
        bind:
          - target-host
        state:
          $:
            opts:
              - present
              - dry-run
            args:
              name:
                - /opt/new-service/conf

Returning Data
--------------

``check``
  Returns directory metadata:

  .. code-block:: json

      {
        "retcode": 0,
        "data": {
          "name": "/etc/myapp",
          "exists": true,
          "is_dir": true,
          "mode": "0755",
          "uid": 0,
          "gid": 0
        }
      }

``present``
  Returns a message describing the action:

  .. code-block:: json

      {
        "retcode": 0,
        "message": "Directory created"
      }

  If the directory already exists with matching attributes:

  .. code-block:: json

      {
        "retcode": 0,
        "message": "Directory already exists with matching attributes"
      }

``absent``
  Returns a message describing the action:

  .. code-block:: json

      {
        "retcode": 0,
        "message": "Directory removed"
      }

  If the directory was not there:

  .. code-block:: json

      {
        "retcode": 0,
        "message": "Directory does not exist"
      }

``dry-run``
  Prefixes the message with ``[dry-run]`` without touching disk:

  .. code-block:: json

      {
        "retcode": 0,
        "message": "[dry-run] would create"
      }

.. note::

    The module is idempotent by design. Running ``present`` twice produces no
    change on the second call. Running ``absent`` on a non-existent directory
    returns success. The ``dry-run`` flag lets you preview any operation
    without risk.
