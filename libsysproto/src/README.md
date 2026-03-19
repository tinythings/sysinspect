# Master/minion protocol

Protocol description about message exchange between master and a minion.

## Secure transport design

This document is the Phase 1 source of truth for the secure Master/Minion transport.
The concrete shared protocol types live in `libsysproto::secure`.

### Transport goals

The secure Master/Minion transport must:

- avoid TLS for Master/Minion links
- avoid DNS assumptions
- tolerate reconnects, address changes, and unstable embedded networks
- keep frames bounded
- reject replayed frames
- support explicit key rotation
- keep only the minimum bootstrap metadata in plaintext
- reject every non-bootstrap plaintext frame once secure transport exists
- allow only one active secure session per minion at a time

### Wire shape

The outer wire shape stays length-prefixed, matching the current transport:

1. write a big-endian `u32` frame length
2. write one serialized secure frame

The secure frame itself is versioned JSON and uses one of four shapes:

- `bootstrap_hello`
- `bootstrap_ack`
- `bootstrap_diagnostic`
- `data`

### Identity binding

Every secure session is bound to all of the following:

- minion id
- minion RSA fingerprint
- master RSA fingerprint
- secure protocol version
- connection id
- client nonce
- master nonce

This binding is represented by `SecureSessionBinding` and must be authenticated during bootstrap.

### Plaintext bootstrap frames

Only these frames may ever be plaintext:

#### `bootstrap_hello`

Sent by the minion to begin a new secure session.

Fields:

- `binding`: initial `SecureSessionBinding`
- `session_key_cipher`: fresh symmetric session key encrypted to the master's registered RSA key
- `binding_signature`: minion RSA signature over the binding and raw session key
- `key_id`: optional transport key identifier for reconnect or rotation continuity

#### `bootstrap_ack`

Sent by the master after validating the registered minion RSA identity and accepting the new secure session.

Fields:

- `binding`: completed `SecureSessionBinding` with the master nonce filled in
- `session_id`: master-assigned secure session id
- `key_id`: accepted transport key id
- `rotation`: `none`, `rekey`, or `reregister`
- `binding_signature`: master RSA signature over the completed binding and accepted session id

#### `bootstrap_diagnostic`

Plaintext rejection or negotiation failure emitted before a secure session exists.

Fields:

- `code`: `unsupported_version`, `bootstrap_rejected`, `replay_rejected`, `rate_limited`, `malformed_frame`, or `duplicate_session`
- `message`: human-readable diagnostic
- `failure`: retry and disconnect semantics

### Encrypted steady-state frame

After bootstrap succeeds, every Master/Minion frame must use `data`.
No other plaintext frame is valid anymore on that connection.

#### `data`

Fields:

- `protocol_version`: secure transport version
- `session_id`: established secure session id
- `key_id`: active transport key id
- `counter`: monotonic per-direction counter
- `nonce`: libsodium nonce for the sealed payload
- `payload`: authenticated encrypted payload

### Failure semantics

Unsupported or malformed peers must not silently fall back to plaintext behavior.

Rules:

- if it is safe to do so, emit `bootstrap_diagnostic`
- disconnect after the diagnostic
- rate-limit malformed bootstrap attempts
- reject duplicate active sessions for the same minion
- reject any post-bootstrap plaintext frame immediately

### Session semantics

Rules:

- only one active secure session may exist per minion
- reconnects must create a new connection id and fresh nonces
- replay protection is per direction and tied to the session id and active key id
- RSA remains only the bootstrap and rotation trust anchor
- steady-state traffic uses libsodium-protected frames only

## Message Structure

### Master message structure

The following is a message structure for a master:

```json
{
    // Target         Destinations
    "t":              [],

    // Request        Type
    "r":              "<type>",

    // Payload        Data in base64 (anything)
    "d":              "<base64>",

    // Return code    int
    "c":              0,
}
```

The following types for "`r`" are available:

- `add` — Minion registration request. Payload contains an RSA public key of a master.
- `cmd` — A regular command to a minion(s).
- `tr` — Request to return all minion traits for the database sync (payload is empty)
or push new (payload exists). This must be used together with the targeting.
- `rm` — Minion un-registration.

## Targeting

Type `t` (target) is a list of target structures. A target structure can target minions
by the following criterias:

1. Hostnames with UNIX type globbing. E.g.: `web*.com`.
2. Machine Id
3. Traits (any)

### Target Structure

```json
{
    // Trait          Targeted traits
    "t":              {},

    // Minion Id      List of minion Ids
    "id":             [],

    // Hostnames      List of minion hostnames
    "h":              [],
}
```

Example targeting by an IPv4 trait, using globbing:

```json
{
    "t": {"system.net.*.ipv4": "192.168.*"},
}
```

Example targeting by an Id:

```json
{
    "id": "30006546535e428aba0a0caa6712e225",
}
```

Example targeting by hosts, starting their domain names as "web":

```json
{
    "h": "web*",
}
```

Example targeting all minions:

```json
{
    "h": ["*"],
}
```

### Minion message structure

The following is a message structure for a minion:

```json
{
    // Id             Machine id or pre-generated equivalent if none
    "id":             "<machine-id>",

    // Request        Type
    "r":              "<type>",

    // Payload        Data in base64 (anything)
    "d":              "<base64>",

    // Return code    int
    "c":              0,
}
```

The following request types for "`r`" are available:

- `add` — Minion registration context. Payload contains nothing.
  In this case Master responds with `add` request, containing its RSA public key.
  A Minion needs to accept it by a fingerprint.

- `rsp` — A regular response to any command.
- `ehlo` — Hello notice for a newly connected minion (any). Contains Minion Id RSA cipher.

## Types

### Request/Response

- `add` — Add a minion, registration request.
- `rm` — Remove a minion, un-registration.
- `rsp` — Regular response to any Master command.
- `cmd` — Regular command to any Minion.
- `tr` — Request to return all minion traits.
- `ehlo` — Hello message to initiate protocol.
- `retry` — Retry connect (e.g. after the registration).
- `pi` — Ping request.
- `po` — Pong response.
- `undef` — Unknown agent.

### Return Codes

- `Undef`: 0 — No specific return code or code is ignorable.
- `Success`: 1 — Successfully completed the routine.
- `GeneralFailure`: 2 — General failure, unspecified. Equal to 1 of POSIX.
- `NotRegistered`: 3 — Minion is not registered. Registration sequence required.
- `AlreadyRegistered`: 4 — Minion is already registered.
- `AlreadyConnected`: 5 — Minion connection duplicate.
- `Unknown`: N/A — Internal designator of unrecognised incoming error code.

## Hello (ehlo)

This sequence requires no established connection.

1. Master listens.
2. Minion sends type `ehlo` request with no payload.
3. Master checks if the Id is registered.
4. In case there is no Id registered, Master responds with the error code and kills
the connection.
5. Master responds with a non-zero return code, mapped to a successful connection.

## Minion Registration Sequence

This sequence requires no established connection.

1. Master listens.
2. Minion sends a request type `add` with an empty payload.
3. Master checks if the Id is registered.
4. In case there is an Id registered, Master responds with the error code and kills
the connection, awaiting `ehlo` instead.
5. Master sends type `add` response with RSA public key.
6. Minion accepts the key, storing it and responds with type `ehlo`, containing own
Id within RSA cipher, using Master's public key.
7. Master responds with a non-zero return code, mapped to a successful connection.

## Minion Call

This sequence requires established successful connection.

1. Master broadcasts type `cmd` to all minions with the destination mask.
2. Each minion accepts the message and looks if a target matches it.
3. Each Minion responds back with type `rsp`.
