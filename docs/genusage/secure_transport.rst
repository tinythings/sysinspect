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
