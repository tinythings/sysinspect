Managed transport metadata
==========================

This module stores managed Master/Minion transport metadata for the secure
protocol work.

Design rules
------------

1. RSA remains the registration and rotation trust anchor.
2. Steady-state libsodium session keys are ephemeral per connection.
3. Operators should not copy raw transport key material around by hand.
4. Registration should auto-provision transport metadata by default.
5. Explicit admin approval workflows must still be representable in metadata.

Managed paths
-------------

Minion:

- `transport/master/state.json`

Master:

- `transport/minions/<minion-id>/state.json`

State contents
--------------

Each state file records:

- bound minion id
- master RSA fingerprint
- minion RSA fingerprint
- secure protocol version
- key exchange model
- provisioning mode
- approval timestamp
- active and last key ids
- last handshake time
- rotation status
- per-key lifecycle metadata

Automation behavior
-------------------

- Minion startup refreshes local transport metadata when a trusted master
  public key already exists locally.
- Master registration refreshes per-minion transport metadata automatically.
- Master startup backfills transport metadata for already-registered minions.

Approval and rotation
---------------------

The default provisioning mode is `automatic`.
The default key exchange model is `ephemeral_session_keys`.

That means:

- each secure Master/Minion session gets a fresh libsodium session key
- RSA bootstraps and authenticates that session key
- no long-lived libsodium steady-state secret is persisted today
- if persisted transport keys are ever introduced later, they must be
  relationship-specific to one master/minion pair

Future admin workflows may switch a peer state to `explicit_approval` and
later approve activation without changing the on-disk layout.
