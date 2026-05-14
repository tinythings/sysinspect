``sys.user``
=============

.. note::

    This document describes ``sys.user`` module usage.

Synopsis
--------

``sys.user`` manages user accounts and groups directly against the system
password and group databases. It reads and writes ``/etc/passwd``,
``/etc/group`` and ``/etc/shadow`` (or ``/etc/master.passwd`` on FreeBSD)
using raw file I/O — no ``useradd``, no ``pw``, no ``dscl``. It works on
truly minimal systems with nothing but a kernel and a filesystem.

Every operation inspects before it acts: ``present`` checks the passwd
file before creating, ``absent`` checks before removing. All operations
return structured telemetry suitable for model conditions and
OpenTelemetry forwarding.

Platform File Paths
-------------------

+-----------+-------------------+------------------+
| Platform  | Passwd / Group     | Shadow           |
+===========+===================+==================+
| Linux     | ``/etc/passwd``    | ``/etc/shadow``  |
|           | ``/etc/group``     |                  |
+-----------+-------------------+------------------+
| FreeBSD   | ``/etc/passwd``    | ``/etc/master.   |
|           | ``/etc/group``     | passwd``         |
+-----------+-------------------+------------------+
| OpenBSD   | ``/etc/passwd``    | ``/etc/master.   |
| NetBSD    | ``/etc/group``     | passwd``         |
| macOS     |                   |                  |
+-----------+-------------------+------------------+
| Solaris   | ``/etc/passwd``    | ``/etc/shadow``  |
|           | ``/etc/group``     |                  |
+-----------+-------------------+------------------+

Usage
-----

  ``check`` — Inspect a user. Returns uid, gid, home, shell, group
  memberships. Always retcode 0, pure telemetry.

  ``present`` — Ensure a user exists. If the user is already present
  with matching attributes (uid, gid, home, shell), the operation is
  a no-op with ``changed: false``. Otherwise the passwd entry is
  created or updated, a matching group is ensured, and the home
  directory is created with correct ownership.

  ``absent`` — Ensure a user does not exist. Removes the passwd entry
  and cleans up group memberships. Optionally removes the home
  directory (``remove-home`` flag).

  ``group-present`` — Ensure a group exists with the given gid.

  ``group-absent`` — Ensure a group does not exist.

  ``dry-run`` — Print what *would* be done without touching any file.

Arguments
---------

  ``name`` (string, required)
    Username or group name.

  ``uid`` (int, optional)
    Desired UID. If not specified during ``present``, the first free
    UID from 1000 upward is used.

  ``gid`` (int, optional)
    Desired primary GID. If not specified, equals the UID. For
    ``group-present``, a free GID is chosen if omitted.

  ``home`` (string, default ``/home/<name>``)
    Home directory path. Created with ``mkdir -p`` and chowned to the
    user if it does not exist.

  ``shell`` (string, default platform-specific)
    Login shell. Defaults to ``/bin/sh`` on BSDs, ``/bin/bash`` on
    Linux and macOS.

  ``groups`` (string, comma-separated, optional)
    Secondary groups the user should be a member of.

  ``password`` (string, optional)
    Password hash or plaintext. If set, written to the shadow file.

  ``remove-home`` (bool, optional)
    When removing a user with ``absent``, also delete the home
    directory and its contents.

  ``mode`` (string, default ``easy``)
    ``strict`` or ``easy``. In strict mode, file write failures
    return non-zero retcode.

Examples
--------

Inspect a user:

.. code-block:: yaml

    actions:
      check-nginx-user:
        module: sys.user
        bind:
          - target-host
        state:
          $:
            opts:
              - check
            args:
              name:
                - nginx

Ensure a system user exists for an application:

.. code-block:: yaml

    actions:
      create-myapp-user:
        module: sys.user
        bind:
          - target-host
        state:
          $:
            opts:
              - present
            args:
              name:
                - myapp
              uid:
                - 501
              home:
                - /opt/myapp
              shell:
                - /sbin/nologin

Remove a user and their home directory:

.. code-block:: yaml

    actions:
      delete-olduser:
        module: sys.user
        bind:
          - target-host
        state:
          $:
            opts:
              - absent
            args:
              name:
                - olduser
              remove-home:
                - true

Ensure a group exists:

.. code-block:: yaml

    actions:
      ensure-dev-group:
        module: sys.user
        bind:
          - target-host
        state:
          $:
            opts:
              - group-present
            args:
              name:
                - developers
              gid:
                - 1001

Quick Test
----------

Inspect the root user:

.. code-block:: sh

    echo '{"options":["check"],"arguments":{"name":"root"}}' | target/debug/user | jq .

Dry-run creating a user:

.. code-block:: sh

    echo '{"options":["present","dry-run"],"arguments":{"name":"myapp","uid":501}}' | target/debug/user | jq .

Returning Data
--------------

``check``
    Returns user telemetry. Always retcode 0.

    .. code-block:: json

        {
          "retcode": 0,
          "data": {
            "name": "nginx",
            "exists": true,
            "uid": 101,
            "gid": 101,
            "home": "/var/empty",
            "shell": "/sbin/nologin",
            "groups": ["www"]
          }
        }

``present``
    Returns whether the user was created or was already present.

    .. code-block:: json

        {
          "retcode": 0,
          "message": "User 'myapp' created (uid=501, gid=501)",
          "data": {
            "name": "myapp",
            "uid": 501,
            "gid": 501,
            "home": "/opt/myapp",
            "shell": "/sbin/nologin",
            "changed": true
          }
        }
