Operator Security Guide
=======================

This page collects the operator-facing secure transport and Web API procedures
in one place.

Registration
------------

When a minion starts without an existing trust relationship, it reports the
master fingerprint and waits for registration.

Typical operator flow:

1. Start ``sysmaster``.
2. Start the minion once and note the reported master fingerprint.
3. Verify that fingerprint out-of-band.
4. Register the minion with ``sysminion --register <master-fingerprint>``.
5. Start the minion normally with ``sysminion --start``.

After registration:

- the master stores the minion RSA public key
- the minion stores the master RSA public key
- both sides create managed transport state on disk
- later reconnects bootstrap secure sessions automatically

Key And State Locations
-----------------------

Under the Sysinspect root, the important files are:

Master:

- ``master.rsa``: master RSA private key
- ``master.rsa.pub``: master RSA public key
- ``console.rsa``: local console private key
- ``console.rsa.pub``: local console public key
- ``console-keys/``: authorised console client public keys
- ``transport/minions/<minion-id>/state.json``: managed transport state for
  one minion

Minion:

- ``minion.rsa``: minion RSA private key
- ``minion.rsa.pub``: minion RSA public key
- ``master.rsa.pub``: trusted master RSA public key
- ``transport/master/state.json``: managed transport state for the current
  master

These files are managed by Sysinspect. Do not edit the transport state files by
hand during normal operation.

Secure Transport Verification
-----------------------------

The quickest operator checks are:

- ``sysinspect network --status``
- master and minion logs

The transport status view shows:

- active key id
- last successful handshake time
- last rotation time
- current rotation state

Healthy behavior looks like:

- the minion reconnects cleanly
- the master logs secure session establishment
- ``network --status`` shows a current handshake timestamp

Transport Key Rotation
----------------------

Rotate one minion:

.. code-block:: bash

   sysinspect network --rotate --id <minion-id>

Rotate a group:

.. code-block:: bash

   sysinspect network --rotate 'edge-*'

Inspect state:

.. code-block:: bash

   sysinspect network --status '*'

Important behavior:

- online minions receive the signed rotation intent immediately
- offline minions keep a pending rotation state until reconnect
- the reconnect after rotation establishes a fresh secure session
- old key material is kept briefly as retiring overlap and then removed

Troubleshooting
---------------

Check the logs first.

Common master-side messages:

- secure bootstrap authentication failure
- replay rejection
- duplicate session rejection
- version mismatch or malformed bootstrap rejection
- staged rotation key mismatch
- Web API TLS startup failure

Common minion-side messages:

- missing trusted master key
- missing managed transport state
- bootstrap diagnostic returned by the master
- bootstrap ack verification failure
- reconnect triggered after transport failure

Typical recovery paths:

- stale or broken minion trust: re-register the minion
- changed master identity: re-register affected minions
- pending rotation on an offline minion: let it reconnect, then check
  ``network --status``
- bad Web API TLS file paths: fix ``api.tls.*`` and restart ``sysmaster``

Web API TLS Setup
-----------------

The embedded Web API is separate from the Master/Minion secure transport.
It uses normal HTTPS/TLS.

Required configuration:

.. code-block:: yaml

   config:
     master:
       api.enabled: true
       api.tls.enabled: true
       api.tls.cert-file: etc/web/api.crt
       api.tls.key-file: etc/web/api.key

Optional configuration:

.. code-block:: yaml

   config:
     master:
       api.tls.ca-file: trust/ca.pem
       api.tls.allow-insecure: true

If ``api.tls.ca-file`` is set, the Web API requires client certificates signed
by that CA bundle.

If the configured Web API certificate is self-signed, set
``api.tls.allow-insecure: true`` only when you intentionally want to allow
that certificate posture. Sysinspect will log a warning when it starts in that
mode.

Behavior:

- if ``api.enabled`` is ``true`` but TLS is not configured correctly, the Web
  API stays disabled
- ``sysmaster`` itself keeps running
- Swagger UI is served over ``https://<host>:4202/doc/`` when ``api.doc`` is ``true``

Operator guidance:

- with ``api.doc: false``, ``/doc/`` and ``/api-doc/openapi.json`` return
  ``404 Not Found``
- with a self-signed Web API certificate, browsers and tools must explicitly
  trust that certificate before Swagger UI can load
- with ``api.tls.ca-file`` configured, the same trusted client certificate is
  required for both the Web API and Swagger UI
- if a client certificate is missing or untrusted, the TLS handshake fails and
  the documentation page is not served
- keep ``api.devmode: false`` for production systems

Re-Registration And Replacement
-------------------------------

Use re-registration when:

- the minion was rebuilt or replaced
- the master identity changed
- trust files were lost or corrupted

A clean replacement flow is:

1. unregister the old minion identity from the master if needed
2. start the replacement minion once and verify the current master fingerprint
3. register the replacement minion
4. start it normally
5. confirm secure handshake and transport status

Related Material
----------------

- :doc:`secure_transport`
- :doc:`transport_protocol`
- :doc:`security_model`
- :doc:`../apidoc/overview`
