``fs.file``
===========

.. note::

    This document describes ``fs.file`` module usage.

Synopsis
--------

The ``fs.file`` module is your go-to tool for anything file-related. It handles
the obvious stuff — creating empty files, copying them around, deleting them,
and peeking at their metadata. But it also goes deeper: you can ensure a
specific line lives in a config file (and it won't touch anything if the line
is already there), strip out lines you no longer want, or replace patterns
across the whole file, much like ``sed`` but with the comfort of knowing the
module checks first and acts second.

Think of it as a polite sysadmin: it looks before it leaps. If the desired
state already exists, it tells you so and moves along. No unnecessary writes,
no busted timestamps, no drama.

Usage
-----

The following options are available:

  ``create``
    Create an empty file with possible content. If ``pull`` is specified, the
    content comes either from a local path (``file:///...``) or from the master
    fileserver.

  ``delete``
    Delete a specified file. In strict mode, returns an error if the file does
    not exist. In easy mode, it simply reports the fact and carries on.

  ``info``
    Get file metadata: path, type, size, timestamps, permissions, owner, group
    and a SHA-256 checksum of the contents. Read-only, never touches the file.

  ``line-present``
    Ensure a specific line exists in the file. If the line is already there
    (exact match), the module smiles and moves on. Otherwise it appends the
    line at the end. If the file does not exist, easy mode creates it with
    that single line; strict mode returns an error.

  ``line-absent``
    Ensure a specific line is absent from the file. Removes *every* matching
    line. If nothing matches, no changes are made. A missing file is considered
    "line already absent" by definition — no error, no change.

  ``replace``
    Replace all occurrences of a pattern with a value. Operates on each line
    independently. If no line contains the pattern, the file is left untouched.
    In strict mode, a missing file is an error.

All content operations (``line-present``, ``line-absent``, ``replace``)
follow the inspect-then-act pattern: the file is read first, and the write
happens only when something actually needs to change.

The following keyword arguments are available:

  ``name`` (type: string, required)
    A target filename. Required for every operation.

  ``mode`` (type: string)
    Can be ``strict`` or ``easy`` (default). In strict mode the module returns
    a non-zero retcode when the desired state cannot be reached. In easy mode
    it returns zero with an explanation in the message.

  ``pull`` (type: string)
    Used with ``create``. If it starts with ``file://`` the source is a local
    path. Otherwise it is a filename served by the master's data fileserver.

  ``pattern`` (type: string)
    Required for ``line-present``, ``line-absent`` and ``replace``. The exact
    line (or substring, for ``replace``) to match in the file.

  ``value`` (type: string)
    Required for ``replace``. The replacement string substituted for every
    occurrence of ``pattern``.

Examples
--------

Pull a file from the master fileserver:

.. code-block:: yaml

    actions:
      deploy-group-file:
        module: fs.file
        bind:
          - target-host
        state:
          $:
            opts:
              - create
            args:
              name:
                - /etc/group
              pull:
                - /standard/group

Copy a local file:

.. code-block:: yaml

    actions:
      backup-group-file:
        module: fs.file
        bind:
          - target-host
        state:
          $:
            opts:
              - create
            args:
              name:
                - /backup/etc/group
              pull:
                - file:///etc/group

Ensure a configuration line is present (e.g. hardening SSH):

.. code-block:: yaml

    actions:
      disable-root-login:
        module: fs.file
        bind:
          - target-host
        state:
          $:
            opts:
              - line-present
            args:
              name:
                - /etc/ssh/sshd_config
              pattern:
                - PermitRootLogin no

Remove a deprecated or dangerous configuration line:

.. code-block:: yaml

    actions:
      drop-protocol-1:
        module: fs.file
        bind:
          - target-host
        state:
          $:
            opts:
              - line-absent
            args:
              name:
                - /etc/ssh/sshd_config
              pattern:
                - Protocol 1

Replace a port number across an nginx configuration:

.. code-block:: yaml

    actions:
      switch-nginx-port:
        module: fs.file
        bind:
          - target-host
        state:
          $:
            opts:
              - replace
            args:
              name:
                - /etc/nginx/nginx.conf
              pattern:
                - listen 80
              value:
                - listen 8080

Returning Data
--------------

``create`` / ``delete``
  Returns a message describing the action taken and ``data.changed: true``.

  .. code-block:: json

      {
        "message": "File /etc/group created",
        "retcode": 0,
        "data": { "changed": true }
      }

``info``
  Returns extensive file metadata in the ``data`` section.

  .. code-block:: json

      {
        "data": {
          "changed": true,
          "path": "/etc/passwd",
          "type": "file",
          "is_file": true,
          "is_dir": false,
          "size": 3442,
          "created": "2023-11-14T15:59:13.966561943+00:00",
          "modified": "2023-11-14T15:59:13.966561943+00:00",
          "accessed": "2025-02-13T15:17:01.315542012+00:00",
          "mode": "0644",
          "uid": 0,
          "gid": 0,
          "user": "root",
          "group": "root",
          "sha256": "ee0582f8..."
        }
      }

``line-present``
  Returns ``changed: true`` when a line was appended, ``changed: false`` when
  the line was already there.

  .. code-block:: json

      {
        "retcode": 0,
        "message": "Line added to /etc/ssh/sshd_config",
        "data": { "changed": true }
      }

``line-absent``
  Returns ``changed: true`` when one or more lines were removed, ``changed:
  false`` when nothing needed removal.

  .. code-block:: json

      {
        "retcode": 0,
        "message": "2 line(s) removed from /etc/ssh/sshd_config",
        "data": { "changed": true }
      }

``replace``
  Returns ``changed: true`` when substitutions were made, ``changed: false``
  when the pattern was nowhere to be found.

  .. code-block:: json

      {
        "retcode": 0,
        "message": "3 replacement(s) in /etc/nginx/nginx.conf",
        "data": { "changed": true }
      }

.. note::

    In easy mode (the default), operations that find the world already in order
    return retcode 0 with ``changed: false``. This means you can run the same
    action repeatedly without fear — the second call is a no-op. In strict
    mode, a file-not-found or a state that cannot be reached produces retcode 1.
