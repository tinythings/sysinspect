Master/Minion Protocol
======================

This page describes the exact secure transport used between ``sysmaster`` and
``sysminion``.

It is intentionally lower-level than :doc:`secure_transport`. Use this page
when you need the precise wire shapes, handshake order, or rejection rules.

Frame Envelope
--------------

The outer wire format is length-prefixed:

1. write a big-endian ``u32`` frame length
2. write one JSON-encoded secure frame

Secure frame kinds are:

- ``bootstrap_hello``
- ``bootstrap_ack``
- ``bootstrap_diagnostic``
- ``data``

Handshake Binding
-----------------

Every secure session is bound to:

- minion id
- minion RSA fingerprint
- master RSA fingerprint
- secure protocol version
- connection id
- client nonce
- master nonce

That binding is carried in ``SecureSessionBinding`` and authenticated during
bootstrap.

Bootstrap Sequence
------------------

Normal secure session establishment is:

1. The minion opens a TCP connection to the master.
2. The minion sends ``bootstrap_hello``.
3. The master validates:
   - the minion is registered
   - the stored RSA fingerprints match
   - at least one common secure transport version exists
   - the opening is not stale or replayed
   - there is no conflicting active session for that minion
4. The master replies with:
   - ``bootstrap_ack`` on success, or
   - ``bootstrap_diagnostic`` on failure
5. Both sides derive the same short-lived session key from:
   - the authenticated Curve25519 shared secret
   - the completed binding
   - both ephemeral public keys
6. After that, every frame on that connection must be ``data``.

``bootstrap_hello``
-------------------

Fields:

- ``binding``: initial session binding
- ``supported_versions``: secure transport versions supported by the minion
- ``client_ephemeral_pubkey``: minion ephemeral Curve25519 public key
- ``binding_signature``: minion RSA signature over the authenticated opening
- ``key_id``: optional managed transport key id for reconnect/rotation continuity

Example:

.. code-block:: json

   {
     "kind": "bootstrap_hello",
     "binding": {
       "minion_id": "minion-a",
       "minion_rsa_fingerprint": "minion-fp",
       "master_rsa_fingerprint": "master-fp",
       "protocol_version": 1,
       "connection_id": "conn-1",
       "client_nonce": "client-nonce",
       "master_nonce": "",
       "timestamp": 1734739200
     },
     "supported_versions": [1],
     "client_ephemeral_pubkey": "<base64>",
     "binding_signature": "<base64>",
     "key_id": "trk-current"
   }

``bootstrap_ack``
-----------------

Fields:

- ``binding``: completed binding with the master nonce filled in
- ``session_id``: master-assigned secure session id
- ``key_id``: accepted transport key id
- ``rotation``: ``none``, ``rekey``, or ``reregister``
- ``master_ephemeral_pubkey``: master ephemeral Curve25519 public key
- ``binding_signature``: master RSA signature over the authenticated ack

Example:

.. code-block:: json

   {
     "kind": "bootstrap_ack",
     "binding": {
       "minion_id": "minion-a",
       "minion_rsa_fingerprint": "minion-fp",
       "master_rsa_fingerprint": "master-fp",
       "protocol_version": 1,
       "connection_id": "conn-1",
       "client_nonce": "client-nonce",
       "master_nonce": "master-nonce",
       "timestamp": 1734739200
     },
     "session_id": "sid-1",
     "key_id": "trk-current",
     "rotation": "none",
     "master_ephemeral_pubkey": "<base64>",
     "binding_signature": "<base64>"
   }

``bootstrap_diagnostic``
------------------------

This is the only failure frame allowed before a secure session exists.

Fields:

- ``code``: ``unsupported_version``, ``bootstrap_rejected``,
  ``replay_rejected``, ``rate_limited``, ``malformed_frame``, or
  ``duplicate_session``
- ``message``: human-readable rejection reason
- ``failure``: retry/disconnect semantics

Example:

.. code-block:: json

   {
     "kind": "bootstrap_diagnostic",
     "code": "replay_rejected",
     "message": "Secure bootstrap replay rejected for minion-a",
     "failure": {
       "retryable": false,
       "disconnect": true,
       "rate_limit": true
     }
   }

``data``
--------

After bootstrap succeeds, all traffic uses ``data``.

Fields:

- ``protocol_version``: negotiated secure transport version
- ``session_id``: active secure session id
- ``key_id``: active managed transport key id
- ``counter``: per-direction monotonic counter
- ``nonce``: counter-derived libsodium nonce
- ``payload``: authenticated encrypted payload

Example:

.. code-block:: json

   {
     "kind": "data",
     "protocol_version": 1,
     "session_id": "sid-1",
     "key_id": "trk-current",
     "counter": 1,
     "nonce": "<base64>",
     "payload": "<base64>"
   }

Enforcement Rules
-----------------

The transport fails closed.

Important rules:

- unsupported peers do not fall back silently
- plaintext registration remains the only allowed non-secure setup path
- plaintext ``ehlo`` and other normal minion traffic are rejected
- duplicate secure sessions for the same minion are rejected
- replayed bootstrap openings are rejected
- replayed, duplicated, stale, or tampered ``data`` frames are rejected
- reconnects create a new connection id, new nonces, and a fresh short-lived
  session key

Rotation Interaction
--------------------

Managed transport rotation does not change the wire shape.

What changes during rotation:

- the active ``key_id`` can change
- the master may advertise rotation state in ``bootstrap_ack``
- reconnect after rotation establishes a fresh secure session using the new
  managed transport key id

Related Material
----------------

- :doc:`secure_transport` for operator-facing usage
- :doc:`operator_security` for registration, key storage, and troubleshooting
- :doc:`security_model` for threat coverage and limits
