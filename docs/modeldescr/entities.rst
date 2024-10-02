Entities
========

.. note::
   This document describes entities definition

The element ``entitles`` is the basis of the model. It contains the entire inventory
of a system in a CMDB fashion.

Each entity can be described in a specific manner, holding all the
necessary attributes, facts, claims and other data that must be
understood by the corresponding consumers. Consumers are actions that
call specific modules, and constraints that process them.

.. important::

   Entities the the following rules:

   - An entity is independent of the specific architecture of a system
   - An entity may contain only self-applied facts, describing only that particular entity.
   - A single entity exists on its own and is unaware of other entities.
   - A compound entity exists only when all its parts are active.

Synopsis
--------

Entitles describe facts and relations within the architecture. These expectations should
be aligned with constraints. Each entity has a **true** or **false** state. A "true" state is when
all constraints and checks produce the expected result.


Each entity is a map. A map starts with an "id" and then contains the necessary attributes.
The current attributes of an "entity" are as follows:

1. ``facts`` *(required)* contains all data to be consumed by any module, check, or constraint, which must be true in the end.
2. ``consists`` *(optional)* It is only for collection entities (e.g., a network) and contains a list of other single entities that together form such a collection. This determines the operational state of the entity **itself**.
3. ``depends`` *(optional)* It is for defining which other entities are required for this entity to be functional.

The data is consumed by a module according to the defined behavior of relations, actions, and constraints.
Its content must be understood by the module.

Detailed Syntax
^^^^^^^^^^^^^^^

Here is the full entity description:

.. code-block::  text

   entities:
     <entity-id>:
        inherits:
          - <entity-id>
        facts:
          <state-id>:
            <label>:
              key: value


``<entity-id>``

  The *entity-id* is an unique ID to place an entity within the namespace.

``inherits`` (optional)

  List of inherited (copied) facts from other entities. Facts section will be just
  merged (overwritten) over the inherited ones with "last wins" rule.

  .. warning::

    The big limitation of the inheritance is that the facts's keys must be unique.
    Otherwise they will clash and overwrite each other. However, often this is desired
    behavior.

``state-id``

  The *state-id* is an ID within a current fact and keeps properties that could match that state. For example, it can hold a data
  for a router with two states: 2.4GHz with 5GHz Wifi and only 2.4GHz Wifi.

``<label>``

  *Label* is a cathegory or group of claims within a specific entity state.

.. hint::

   State-id and label are for constructing data under different modes of an entity and calling a corresponding module accordingly.

An example of a single entities:

.. code-block:: yaml

  entities:
    journald:
      facts:
        default:
          - label:
            path: /lib/systemd/systemd-journald

An example of a compound entity:

.. code-block:: yaml

  logging:
    descr: Subsystem that allows system logging
    depends:
      - journald
      - syslogd
      - systemd

An entity can be also just a static configuration of something, keeping facts.
For example:

.. code-block:: yaml

  entities:
    systemconf:
      descr: static system configuration
      facts:
        default:
          - main:
              storage:
              type: SSD
              size: 2TB
              free: 500Mb
            mem:
              free: 10Mb

All of these entities describe something: a process, an ECU with its APIs, an application, a service,
a collection of those entities, or even just a physical wire. With this in mind, a network is an entity,
but it is a compound one, where one can "zoom in" to see its smaller parts, which can also be compound
entities representing some part of the network, and so on.

Facts
^^^^^

Each entity **must** contain some facts about it.

A section in key/value format contains a series of facts under the name ``facts``. Each *fact* consists of *claims*,
and a fact can have one or more claims. Facts also have states. States are essentially the segregation of
facts, by which constraints and actions select different parameters for the processing module.

Syntax:

.. code-block:: text

   facts:
     <id>:
      <state>:
        key: value

Each fact has a label, which allows it to be tagged so that any other process can refer to this
particular fact directly or indirectly. The main use of labels is in declarative constraints.

Here is an example of a fact that claims there is a TCP network with an open SSH port,
listening to the world:

.. code-block:: yaml

   facts:

      # Fact ID or label. It is unique per
      # facts set within the entity.
      # The label isn't addressed and skipped.
      tcp-network:

         # State ID by which action may refer it
         default-state:

            # Fact label
            label:

              # Here are whatever key/value data, understandable by a
              # corresponding plugin.
              type: tcp
              port: 0.0.0.0:22
              listen: 0.0.0.0:*

A fact's claims are just arbitrary key/value pairs that can later be referred to by a
corresponding consumer, such as a logic flow, an action, a plugin, etc.

Facts can be addressed by built-in functions directly or indirectly:

.. code-block:: yaml

   # Directly
   foo: static(entitles.ssh-sockets.facts.port)

   # Indirectly, within the context of a current fact
   # this returns the whole fact structure by its static ID
   bar: fact(tcp-network)

   # Claim returns a specific value of a claim within a current fact
   baz: claim(port)

For more details about fact functions, please refer to the corresponding section.
