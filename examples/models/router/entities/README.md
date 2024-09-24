# Entities

`Entitles` are any inventory objects in CMDB that can be described
in a specific manner, holding all the necessary attributes, those
are understood by corresponding modules.

## Rules

The following rules are applied to an entity:

- It is ambiguous to the specific architecture
- It contains only self-applied facts
- It doesn't know about other entities

## Synopsis

Entitles describing facts and relations within the architecture.
These expectations should be aligned to constraints. Each entity
has **true** or **false** state. A "true state" is a state, where
all constraints and checks come to an expected result.

Each entity is a map. A map just starts as an "id" and then contains
needed attributes. Current attributes of an "entity" as follows:

1. `facts` _(required)_ contains all data to be consumed by any module
or check or a constraint, that must be true at the end.
2. `consists` _(optional)_ is only for collection entities (e.g. network)
and it contains a list of other single entities that would make together
such collection.

The data is consumed by a module, according to the defined behaviour
of relations, actions and constraints. Its content must be understood
by a module.

In the following example is the `process` entity with the ID `systemd`:

An example of a single entities:

```yaml
entities:
  - journald:
    facts:
      path: /usr/bin/journald

  - ports:
    facts:
      - network: tcp
        port: 0.0.0.0:22
        listen: 0.0.0.0:*

      - network: udp
        port: 0.0.0.0:8181
        listen: 0.0.0.0:*

  - routes:
    facts:
      - 192.168.1.5/24
```

An example of a collection entity:

```yaml
entities:
  - network:
    consists:
      - ports
      - routes
```

## Facts

THe `facts` section is a key/value container of facts. A fact consists
of claims, and it can consists of one or more claims. For example, a
fact claims that there is TCP network with opened SSH port, listening
to the world:

```yaml
facts:
  # Fact ID or label. It is unique per
  # facts set within the entity.
  # The label isn't addressed and skipped.
  tcp-network:
    # Here are whatever key/value data, understandable by a
    # corresponding plugin.
    type: tcp
    port: 0.0.0.0:22
    listen: 0.0.0.0:*
```

Facts are always addressed directly (full path or contextual):

```yaml
# full
foo: $(entitles.ssh-sockets.facts.port)

# context
bar: @(facts.port)
```
