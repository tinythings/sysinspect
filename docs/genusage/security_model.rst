Security Model
==============

This page states what each security layer in Sysinspect is responsible for.

Master/Minion Transport
-----------------------

For the Master/Minion link:

- RSA identifies the master and the minion
- RSA signatures authenticate the bootstrap exchange
- ephemeral Curve25519 key exchange provides a fresh short-lived shared secret
- libsodium protects steady-state traffic after bootstrap

What this gives you:

- authenticated peers
- fresh per-connection session protection
- replay rejection through counters and nonce derivation
- fail-closed rejection of unsupported or malformed peers

What it is not:

- browser-style TLS
- a public PKI system
- a substitute for operator verification of the initial trust relationship

Web API
-------

For the embedded Web API:

- TLS protects the HTTPS connection
- bearer tokens authenticate API requests
- request and response bodies are plain JSON over HTTPS

What this gives you:

- standard HTTPS protection for remote API calls
- standard certificate validation behavior unless explicitly relaxed by the
  client side
- no custom application-layer crypto inside the JSON payloads

What it is not:

- part of the Master/Minion secure transport
- protected by the Master/Minion libsodium channel

Console
-------

The local ``sysinspect`` to ``sysmaster`` console path is a separate transport.

It remains:

- local to the master host
- based on its own console RSA/bootstrap mechanism
- independent from both the Web API TLS layer and the Master/Minion transport

Threats Covered
---------------

The current design is meant to cover:

- passive eavesdropping on Master/Minion traffic after registration
- tampering with secure Master/Minion frames
- replay of old secure frames
- unsupported peers attempting insecure fallback
- duplicate active sessions for the same minion
- normal remote API exposure through plaintext HTTP

Out Of Scope
------------

The current design does not try to solve everything.

Out of scope:

- compromise of the master or minion host itself
- theft of private keys from a compromised host
- manual trust mistakes during initial fingerprint verification
- deliberate operator choice to allow self-signed Web API TLS
- weak client-side trust settings outside Sysinspect server configuration
- generic browser, PAM, LDAP, or operating-system hardening outside
  Sysinspect itself

Operational Assumptions
-----------------------

Sysinspect assumes:

- the initial master fingerprint is verified by the operator
- private key files are protected by host filesystem permissions
- managed transport state is not edited manually during normal operation
- Web API certificates and keys are provided and rotated through normal
  operator procedures

In short:

- RSA authenticates identities
- Curve25519 + libsodium protect Master/Minion traffic
- TLS protects the Web API
- each layer has a separate role and should be operated that way
