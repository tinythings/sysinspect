``cfg.resource``
================

.. note::

    This document describes ``cfg.resource`` module usage.

Synopsis
--------

This module manages shared artifacts (files) via Sysmaster datastore.
It can:

- ``push`` a local file from a Minion into datastore
- ``pull`` a file from datastore and materialise it locally

The module is designed for resource workflows where models only describe
**what** to place and **where**. Connection details and authentication are taken from Minion
runtime configuration behind the scenes.

In simple words for day-to-day usage:

- ``src`` = logical resource id in datastore
- ``file``/``dst`` = local filesystem path

Usage
-----

The following options are available:

  ``push``
    Upload local file to datastore under the logical id from ``src``.

  ``pull``
    Resolve the logical id from ``src``, download matching artifact and place it locally.

  ``force``
    Disable checksum skip logic. Always push/pull even if data already matches.


The following keyword arguments are available:

  ``src`` (type: string, required)
    Logical resource id in datastore.

    Typical examples:

    - ``/somehost/etc/ssh/authorized_keys``
    - ``/someotherhost/etc/ssh/authorized_keys``

  ``file`` (type: string, optional)
    Local filesystem path.
    For ``push`` it is a source file.
    For ``pull`` it is a destination file.

  ``dst`` (type: string, optional)
    Alias for ``file``.

  ``mode`` (type: string, optional)
    File mode for pull result (octal, for example ``0644``).
    If omitted, mode from datastore metadata is applied.


Behavior Notes
--------------

- If ``file`` and ``dst`` are both absent, module uses ``src`` as local path.
- ``pull`` checks local checksum first and skips download when already up to date (unless ``force``).
- ``push`` checks datastore checksum first and skips upload when already up to date (unless ``force``).
- Module transport/auth details are not passed from model arguments.
  They are taken from runtime Minion config injected automatically during module call.


State Semantics
---------------

In Sysinspect, ``state`` is the state of an **entity**, not a generic cfgmgmt flag like
``present``/``absent``.

If you do not want to model a detailed lifecycle (for example ``off -> starting -> on``),
use the wildcard state ``$``.

For ``cfg.resource`` this is usually the right default unless your entity truly has
multiple operational states.


Examples
--------

The following examples are **model DSL** examples, so they are written in YAML.

Pull resource to local file:

.. code-block:: yaml

    actions:
      sync-authorized-keys:
        module: cfg.resource
        bind: [ssh-key-resource]
        state:
          $:
            opts: [pull]
            args:
              src: /somehost/etc/ssh/authorized_keys
              file: /etc/ssh/authorized_keys
              mode: "0600"

Push local file to datastore:

.. code-block:: yaml

    actions:
      publish-authorized-keys:
        module: cfg.resource
        bind: [ssh-key-resource]
        state:
          $:
            opts: [push]
            args:
              src: /somehost/etc/ssh/authorized_keys
              file: /etc/ssh/authorized_keys

Force pull even if checksum matches:

.. code-block:: yaml

    actions:
      force-sync-authorized-keys:
        module: cfg.resource
        bind: [ssh-key-resource]
        state:
          $:
            opts: [pull, force]
            args:
              src: /somehost/etc/ssh/authorized_keys
              dst: /etc/ssh/authorized_keys


Returning Data
--------------

Common response fields:

- ``retcode``: ``0`` on success, non-zero on error
- ``message``: human-readable summary
- ``data.changed``: whether the local/remote state was modified

Typical extra fields in ``data``:

- ``src`` logical id
- ``dst`` local path (for pull)
- ``sha256`` resolved artifact checksum
- ``size_bytes`` artifact size
- ``mode`` resulting local mode (for pull)

The examples below are **module runtime protocol payloads** and are exchanged
between Sysinspect runtime and module process over STDIN/STDOUT, therefore
they are JSON.

Example successful pull response (runtime payload):

.. code-block:: json

    {
      "retcode": 0,
      "message": "Resource downloaded from datastore",
      "data": {
        "changed": true,
        "src": "/somehost/etc/ssh/authorized_keys",
        "dst": "/etc/ssh/authorized_keys",
        "sha256": "9df2...ab12",
        "size_bytes": 1430,
        "mode": "0600"
      }
    }

Example no-change pull response (runtime payload):

.. code-block:: json

    {
      "retcode": 0,
      "message": "Resource '/etc/ssh/authorized_keys' already matches checksum",
      "data": {
        "changed": false
      }
    }
