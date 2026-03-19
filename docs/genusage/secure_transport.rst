Secure Master/Minion Transport
==============================

This page explains, in user-facing terms, how Sysinspect protects traffic
between a Master and its Minions.

For most users, the important point is simple: Master/Minion communication is
secured automatically. You normally do not need to configure the transport by
hand or understand the protocol internals.

This page is only about the Master/Minion link. It does not describe the Web
API.

What You Need To Know
---------------------

- Sysinspect secures traffic between the Master and Minions automatically.
- The Master/Minion link does not use browser-style TLS certificates.
- Trust is based on the identities of the Master and the Minion.
- Session protection is created automatically when they connect.

In normal operation, there is nothing special you need to do beyond registering
the Minion and keeping the trust relationship intact.

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
- refreshing transport state when services start
- tracking the information needed to re-establish trust

You do not need to create or copy transport secrets yourself.

How It Works On The Master
--------------------------

In simple terms, the Master side works like this:

1. The Master has its own RSA identity keypair.
2. When you register a Minion, the Master stores that Minion's public RSA key.
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

How It Works On The Minion
--------------------------

The Minion side follows the same trust relationship from the other direction:

1. The Minion has its own RSA identity keypair.
2. During registration, the Minion learns the Master's public RSA key.
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

What Actually Protects The Traffic
----------------------------------

The protection happens in two steps:

1. RSA identity keys prove who the Master and Minion are.
2. A short-lived secure session protects the normal traffic after bootstrap.

So the long-term trust comes from the registered identities, while everyday
traffic is protected by a fresh session created when the connection starts.

What Operators Should Do
------------------------

For regular administration, the best approach is:

- let Sysinspect manage the transport state
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

In those cases, prefer the supported recovery path such as re-registration or
re-bootstrap instead of editing transport files manually.
