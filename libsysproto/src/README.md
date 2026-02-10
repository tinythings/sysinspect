# Master/minion protocol

Protocol description about message exchange between master and a minion.

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
