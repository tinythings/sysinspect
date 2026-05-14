Secure Master/Minion Transport
==============================

This page explains, in user-facing terms, how Sysinspect protects traffic
between a Master and its Minions.

For most users, the important point is simple: Master/Minion communication is
secured automatically. You normally do not need to configure the transport by
hand or understand the protocol internals.

This page is only about the Master/Minion link. It does not describe the Web
API.

For the exact wire format, see :doc:`transport_protocol`.
For operator procedures, see :doc:`operator_security`.
For threat coverage and limits, see :doc:`security_model`.

What You Need To Know
---------------------

- Sysinspect secures traffic between the Master and Minions automatically.
- The Master/Minion link does not use browser-style TLS certificates.
- Trust is based on the identities of the Master and the Minion.
- Session protection is created automatically when they connect.

In normal operation, there is nothing special you need to do beyond registering
the Minion and keeping the trust relationship intact.

With the current onboarding workflow, the usual operator path is not manual
``sysminion --register`` on the host anymore. The preferred flow is
``sysinspect network --add ...`` on the master side, which performs the
registration and startup sequence for you.

Why This Matters
----------------

Sysinspect is designed to work in environments where ordinary TLS deployment
can be inconvenient, especially on embedded or unstable networks. For example:

- DNS may not exist
- IP addresses may change
- reconnects may happen often

Because of that, Sysinspect uses its own secure Master/Minion transport instead
of requiring a certificate-based TLS setup for every node.

What Sysinspect Manages For You
-------------------------------

Sysinspect keeps transport state on disk so it can maintain the trusted
relationship automatically.

You may see these files:

- on a Minion: ``transport/master/state.json``
- on a Master: ``transport/minions/<minion-id>/state.json``

These files are managed by Sysinspect. They are not meant to be part of normal
day-to-day configuration.

In normal use, Sysinspect handles:

- preparing transport state when a Minion is registered
- persisting the master's RSA public key on the minion during onboarding
- refreshing transport state when services start
- tracking the information needed to re-establish trust

You do not need to create or copy transport secrets yourself.

How It Works On The Master
--------------------------

In simple terms, the Master side works like this:

1. The Master has its own RSA identity keypair.
2. When a Minion is registered, the Master stores that Minion's public RSA key.
3. The Master also creates managed transport state for that Minion under
   ``transport/minions/<minion-id>/state.json``.
4. When the Minion connects, it starts a secure bootstrap using the already
   trusted identities.
5. The Master checks that the Minion is known, that the fingerprints match,
   and that the bootstrap request belongs to this protocol version.
6. If everything matches, the Master accepts the bootstrap and creates a secure
   session for that TCP connection.
7. After that point, normal Master-to-Minion traffic is sent through the secure
   session instead of plain JSON frames.
8. If the bootstrap is broken, unsupported, or duplicated, the Master rejects
   it and drops the connection.

What this means for an operator:

- registration prepares the trust relationship
- reconnects can create a fresh secure session
- one broken connection does not require you to rebuild keys by hand
- ``network --add`` is expected to create this state automatically

How It Works On The Minion
--------------------------

The Minion side follows the same trust relationship from the other direction:

1. The Minion has its own RSA identity keypair.
2. During registration, the Minion learns the Master's public RSA key and
   persists it on disk.
3. The Minion stores managed transport state under
   ``transport/master/state.json``.
4. On normal startup, the Minion loads that managed state and starts a secure
   bootstrap before it begins processing normal Master commands.
5. If the Master accepts the bootstrap, the Minion switches the connection to a
   secure session.
6. Commands, pings, trait updates, events, and other normal traffic then use
   that secure session automatically.
7. If the secure bootstrap fails, the Minion reconnects instead of silently
   continuing on an insecure path.
8. If the Minion no longer has trusted transport data, it does not continue in
   plain mode. It stops and waits for secure recovery, re-bootstrap, or
   re-registration.

What this means for an operator:

- a healthy Minion should secure the connection automatically on startup
- replacing or re-registering a Minion may require a fresh trust relationship
- if trust data is stale, recovery should use re-bootstrap or re-registration,
  not hand-edited files
- if the Minion cannot prove trust anymore, it should fail closed rather than
  quietly continue insecurely

Current operator-ready onboarding path
--------------------------------------

The preferred production workflow is:

1. Ensure SSH to the target already works.
2. Publish the desired ``sysminion`` build in the repository.
3. Run ``sysinspect network --add ...`` from the master host.
4. Let the master:

   - choose the matching remote minion artefact
   - run remote setup
   - read the master's RSA public key from local disk
   - derive the master fingerprint from that key
   - register the remote minion against that fingerprint
   - wait for secure bootstrap and full readiness

This matters because the trust-seeding source of truth is the master's own RSA
public key on disk, not a copied fingerprint string that the operator typed by
hand elsewhere.

What Actually Protects The Traffic
----------------------------------

The protection happens in two steps:

1. RSA identity keys prove who the Master and Minion are.
2. A short-lived secure session protects the normal traffic after bootstrap.

So the long-term trust comes from the registered identities, while everyday
traffic is protected by a fresh session created when the connection starts.

What Changes And What Does Not
------------------------------

This transport change affects the Master/Minion boundary only.

What changes:

- the Master/Minion bootstrap now uses authenticated ephemeral key exchange
- reconnects always create a fresh secure session
- unsupported or malformed peers fail explicitly instead of falling back

What does not change:

- the local ``sysinspect`` to ``sysmaster`` console path still uses its own
  local console transport
- the embedded Web API still uses normal HTTPS/TLS and is separate from the
  Master/Minion transport
- the fileserver still publishes artefacts on its existing fileserver endpoint
- profile assignment still happens through master-managed traits and normal
  sync workflows

Operationally this means:

- console administration commands keep working as before
- Web API TLS settings are configured separately under ``api.*``
- ``sysinspect --sync`` still refreshes modules, libraries, sensors, and
  profiles through the fileserver path after the secure control channel is up
- profile sync policy is unchanged; the secure transport only protects the
  control messages that trigger or coordinate it

What Operators Should Do
------------------------

For regular administration, the best approach is:

- let Sysinspect manage the transport state
- prefer ``sysinspect network --add/remove/upgrade`` for managed lifecycle work
- use the normal registration or recovery workflow if trust breaks
- check that the Master and Minion still recognize each other as trusted peers

Avoid manual fixes unless you are doing low-level recovery work:

- do not edit ``state.json`` files casually
- do not copy secrets or key material between systems by hand
- do not treat transport state as ordinary configuration

When Something Breaks
---------------------

If a Minion can no longer establish a secure connection, the usual causes are:

- the node was reinstalled or re-registered
- trust metadata is stale
- the Master and Minion no longer agree on identity
- the master's public key was not persisted correctly during onboarding

In those cases, prefer the supported recovery path such as re-registration or
re-bootstrap instead of editing transport files manually.

Operator Diagnostics
--------------------

Sysinspect now emits operator-visible diagnostics for the common failure cases.

Look for these classes of messages:

- secure bootstrap authentication failure
- secure bootstrap replay rejection
- secure bootstrap version mismatch or malformed-frame rejection
- staged rotation key mismatch versus the reconnecting Minion key
- Web API TLS startup failure, including configured cert/key/CA paths

The quickest operator checks are:

- ``sysinspect network --status`` for active key id, last handshake time, and
  rotation state
- ``sysinspect cluster --online`` for current online/offline state
- ``sysinspect network --info <host-or-id>`` for persisted traits and identity
- the master error log for bootstrap rejection and TLS startup messages
- the minion error log for bootstrap-diagnostic and ack-verification failures

If a Minion reconnects but does not complete bootstrap, check the logs on both
sides before editing any managed state.

If ``network --add`` fails after registration but before readiness, Sysinspect
now treats that as a partial onboarding failure and cleans up what it safely
can:

- staged upload directory
- partial runtime on the target
- partial master registration when registration had already happened

Transport Rotation Workflow
---------------------------

Sysinspect supports managed transport-key rotation from the Master control
plane.

For operators, this means:

- you can rotate one target in one command path
- online Minions receive a signed rotation intent immediately
- offline Minions are marked as pending and receive rotation on next reconnect
- old key material is kept briefly as ``retiring`` and then removed after the
  grace overlap window

Typical usage:

- rotate a specific minion id: ``sysinspect network --rotate --id <minion-id>``
- rotate by selector query: ``sysinspect network --rotate '<glob>'``
- inspect transport status: ``sysinspect network --status <minion-id|glob>``

Practical Rotation Procedure
----------------------------

For day-to-day operation, the usual workflow is:

1. Check which Minions are online.
2. Inspect current transport state.
3. Run rotation for one Minion or a selected group.
4. Wait for reconnect/bootstrap.
5. Confirm the new active key and ``last-rotated`` timestamp.

In practice this looks like:

1. Show online Minions:

   .. code-block:: bash

      sysinspect network --online

2. Inspect one Minion before rotation:

   .. code-block:: bash

      sysinspect network --status <minion-id>

3. Rotate that Minion with the default overlap window:

   .. code-block:: bash

      sysinspect network --rotate <minion-id>

4. Inspect it again after it reconnects:

   .. code-block:: bash

      sysinspect network --status <minion-id>

If the Minion is online, Sysinspect sends the signed rotation intent
immediately.

If the Minion is offline, Sysinspect stores the exact requested rotation as a
pending action and sends it when that Minion reconnects later.

What Each Rotation Option Does
------------------------------

The current operator-facing rotation options are:

``network --rotate <target>``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Starts a managed transport rotation.

- If ``<target>`` looks like a plain Minion id, Sysinspect targets that exact
  Minion.
- If ``<target>`` contains glob characters such as ``*`` or a comma-separated
  selector list, Sysinspect treats it as a query target.

Examples:

.. code-block:: bash

   sysinspect network --rotate minion-42
   sysinspect network --rotate 'edge-*'
   sysinspect network --rotate 'edge-1,edge-2'

``network --rotate --rotate-overlap <seconds>``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Controls how long the old key remains in ``retiring`` state before it is
removed.

- Default: ``900`` seconds
- Purpose: give unstable or slow-to-reconnect systems a grace window during
  cutover

Example:

.. code-block:: bash

   sysinspect network --rotate minion-42 --rotate-overlap 1800

That keeps the previous key material around for 30 minutes before retirement
cleanup removes it.

``network --rotate --rotate-reason <text>``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Adds operator-visible context to the signed rotation intent.

- This is recorded in the staged rotation request.
- If the Minion is offline, the same reason is preserved and replayed later.

Example:

.. code-block:: bash

   sysinspect network --rotate minion-42 --rotate-reason quarterly-maintenance

``network --status <target>``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Shows current managed transport state for one Minion or a selected set.

The status output includes:

- active transport key id
- key age
- last successful handshake time
- current rotation state
- ``security.transport.last-rotated-at``

Examples:

.. code-block:: bash

   sysinspect network --status minion-42
   sysinspect network --status 'edge-*'

Related Operational Options
---------------------------

These options are not rotation-specific, but they are commonly used together
with transport-key rotation:

``network --online``
~~~~~~~~~~~~~~~~~~~~

Shows which registered Minions are currently online.

Use this before rotation to decide whether the request is likely to be applied
immediately or deferred until reconnect.

.. code-block:: bash

   sysinspect network --online

``--sync``
~~~~~~~~~~

Synchronises modules, libraries, profiles, and related managed artefacts
across the cluster.

This is separate from transport-key rotation, but operators often run it after
maintenance work when they want Minions to be both current and reconnected.

.. code-block:: bash

   sysinspect --sync

``--unregister <minion-id>``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Removes a Minion's registration and its managed transport metadata from the
Master.

Use this only when you intend to force a fresh trust relationship.

.. code-block:: bash

   sysinspect --unregister minion-42

After unregistering, the Minion must be registered again before normal secure
bootstrap can continue.

What Actually Happens During Rotation
-------------------------------------

From an operator point of view, a rotation request does the following:

1. The Master creates a new managed transport key record with fresh secret
   material.
2. The Master signs a rotation intent with its RSA identity.
3. The Minion verifies that signed intent against the trusted Master RSA key.
4. The Minion updates its managed transport state and reconnects.
5. The next secure bootstrap uses the new managed transport key material.
6. After the overlap window expires, the old key is retired and removed.

This means rotation changes real managed transport secret material, not only a
label or identifier.

Recommended Operator Patterns
-----------------------------

For one Minion:

.. code-block:: bash

   sysinspect network --status minion-42
   sysinspect network --rotate minion-42 --rotate-reason planned-maintenance
   sysinspect network --status minion-42

For a group with a longer grace window:

.. code-block:: bash

   sysinspect network --online
   sysinspect network --rotate 'edge-*' --rotate-overlap 3600 --rotate-reason staged-rollout
   sysinspect network --status 'edge-*'

For offline or unstable Minions:

- issue the same rotation command normally
- let Sysinspect keep the request pending
- verify the result after the Minion reconnects with ``network --status``

The status view includes:

- active transport key id
- key age
- last successful handshake timestamp
- current rotation state
- ``security.transport.last-rotated-at`` value

Fresh Installs, Re-Registration, And Admin Workflows
----------------------------------------------------

The intended operator workflow remains simple:

- fresh registration auto-provisions the managed transport metadata
- normal reconnects auto-bootstrap a fresh secure session
- re-registration replaces the trust relationship when identity changes
- master-side administration stays on the console and Web API paths

In other words, the secure transport is hardened without adding a manual
day-to-day key exchange procedure for operators.

Rotation Safety Model
---------------------

Rotation intents are signed by the Master RSA trust anchor and verified by the
Minion before any transport-state changes are applied.

If message construction fails after state staging on the Master, the Master
restores the previous state automatically. This provides a safe automatic
rollback path without requiring operators to perform manual state surgery.

Unregister Cleanup
------------------

Removing a Minion from the Master removes both:

- the Minion registration RSA artifact
- the Minion managed transport state directory

This keeps registration lifecycle and transport lifecycle consistent.

Disaster Recovery
-----------------

Use managed recovery flows for key loss or metadata corruption.

Lost or Corrupted Minion Transport State
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

1. Recreate trust from the Master side by rotating (or re-registering if
   needed).
2. Restart the Minion to force a fresh secure bootstrap.
3. Confirm status via ``sysinspect network --status <minion-id>``.

Lost Master-Side Transport Metadata For One Minion
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

1. Keep the Minion registration key if still valid.
2. Trigger ``sysinspect network --rotate <minion-id>`` to stage a new signed intent.
3. Let the Minion reconnect and apply the new state.

Master Rebuild Scenario
~~~~~~~~~~~~~~~~~~~~~~~

If the Master identity changes, existing trust bindings are no longer valid.
In that case:

1. Re-establish Master RSA trust according to the registration workflow.
2. Re-register Minions to bind them to the rebuilt Master identity.
3. Run rotation/status checks to verify all nodes have fresh managed transport
   state.
