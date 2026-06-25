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
**what** to place and **where**.

Transport is resolved internally against ``master.ip`` from Minion
configuration and can be controlled explicitly with module args:

- ``tls: true`` => ``https://<master.ip>:<port>``
- ``tls: false`` => ``http://<master.ip>:<port>``

Authentication Model
--------------------

There are two different authentication contexts around ``cfg.resource``:

- **Operator / external Web API access**
  The embedded Web API uses normal HTTPS transport plus bearer-token
  authentication for human or external API clients.

- **Minion-internal resource access**
  ``cfg.resource`` is intended to be used from minion-side model actions.
  In that flow the minion is already trusted by the Master over the normal
  Sysinspect transport, so datastore access should be transparent and must not
  require PAM login prompts or operator bearer-token handling inside models.

The current design for ``cfg.resource`` is therefore:

- keep bearer-token auth for normal external Web API users
- allow datastore endpoints to accept **minion-auth** for minion-originated
  requests
- keep the module itself stateless

Current minion-auth bootstrap shape:

- ``X-Sysinspect-Minion-Id``
- ``X-Sysinspect-Timestamp``
- ``X-Sysinspect-Signature``
- ``X-Sysinspect-Body-Sha256``

The bootstrap signature covers a small canonical request string:

- HTTP method
- request path
- query string
- timestamp
- body hash

The module first authenticates on:

- ``POST /store/auth/minion``

and receives a short-lived datastore bearer token. Subsequent datastore calls
for that module run then use normal ``Authorization: Bearer`` headers, but the
token was bootstrapped transparently from the minion's registered RSA identity.

This keeps datastore access transparent for internal model execution while
still preventing unauthenticated external callers from using the same API.

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

  ``sync-dir``
    Resolve all datastore items whose logical id matches the prefix from ``src``
    and materialise them into a local directory.


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

    For ``sync-dir`` it should point to a destination directory.

  ``mode`` (type: string, optional)
    File mode for pull result (octal, for example ``0644``).
    If omitted, mode from datastore metadata is applied.

  ``tls`` (type: bool, optional)
    Whether to use HTTPS for the datastore API transport.
    Default: ``true``.

  ``tls-accept-insecure`` (type: bool, optional)
    Allow self-signed or otherwise invalid TLS certificates.
    Default: ``false``.

  ``port`` (type: integer, optional)
    Web API port on the master.
    Default: ``4202``.


Behavior Notes
--------------

- If ``file`` and ``dst`` are both absent, module uses ``src`` as local path.
- ``pull`` checks local checksum first and skips download when already up to date (unless ``force``).
- ``push`` checks datastore checksum first and skips upload when already up to date (unless ``force``).
- Only ``master.ip`` comes from runtime Minion config.
- Protocol and API port can be overridden explicitly through ``tls``,
  ``tls-accept-insecure``, and ``port``.
- Operator bearer-token auth is not a suitable fit for internal model calls.
- ``cfg.resource`` therefore bootstraps datastore access from the minion's
  registered RSA identity and then reuses a short-lived datastore bearer token
  automatically.


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
              tls: true
              tls-accept-insecure: true
              port: 4202

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
              tls: true
              tls-accept-insecure: true
              port: 4202

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
              tls: true
              tls-accept-insecure: true
              port: 4202

Sync all public keys from one logical prefix into a local directory:

.. code-block:: yaml

    actions:
      sync-pubring:
        module: cfg.resource
        bind: [shared-keyring]
        state:
          $:
            opts: [sync-dir]
            args:
              src: /keyringdemo/
              dst: /tmp/keyringdemo/pubring
              mode: "0644"
              tls: true
              tls-accept-insecure: true
              port: 4202


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
- ``synced`` number of materialised files (for ``sync-dir``)

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
