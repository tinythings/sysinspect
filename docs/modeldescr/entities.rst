Entities
========

.. note::
   This document describes entities definition

The element ``entitles`` is the basis of the model. It contains all
the inventory of a system in a CMDB fashion.

Each entity can be described in a specific manner, holding all the
necessary attributes, facts, claims and other data which must be
understood by corresponding consumers. Consumers are actions that
are calling specific modules, and constraints that are processing them.

.. important::

   Entities has rules as follows:

   - An entity is ambiguous to a specific architecture of a system
   - Entity may contain only self-applied facts, describing only that particular entity
   - A single entity lives on its own and does not know about other entities
   - A compound entity lives only when all its parts are alive

Synopsis
--------

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

.. code-block:: yaml

   entities:
     - journald:
         facts:
           label:
             path: /lib/systemd/systemd-journald

An example of a compound entity:

.. code-block:: yaml

   - logging:
       descr: Subsystem that allows system logging
       depends:
         - journald
         - syslogd
         - systemd

An entity can be also just a static configuration of something, keeping facts.
For example:

.. code-block:: yaml

   entities:
     - systemconf:
        descr: static system configuration
        facts:
        storage:
           type: SSD
           size: 2TB
           free: 500Mb
        mem:
           free: 10Mb

Aall of these entities are describing something: a process, an ECU with its APIs,
an application, a service, a collection of those entities and even just a physical wire.
With this in mind, a network is an entity, but it is compound one, where one can "zoom in"
and get to the smaller parts of it, those can be also compound entities, representing some
part of a network etc.

Facts
^^^^^

Each entity **must** contain some facts about it.

A section in key/value format contains a series of facts under the name ``facts``. Each *fact*
consists of *claims*. A fact can have one or more claims.

Syntax:

.. code-block:: yaml

   facts:
     <label>:
       key: value

Each fact has a *label* which then allows to tag it, so any other process can refer to this
particular fact directly or indirectly. Main usage of labels are in declarative constraints.

Here is an example of a fact, which claims that there is a TCP network with opened SSH port,
listening to the world:

.. code-block:: yaml

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

Facts's claims are just arbitrary key/value that can be then later referred by a corresponding
consumer, such as a logic flow, an action, a plugin etc.

Facts can be addressed by built-in functions directly or indirectly:

.. code-block:: yaml

   # Directly
   foo: static(entitles.ssh-sockets.facts.port)

   # Indirectly, within the context of a current fact
   # this returns the whole fact structure by its static ID
   bar: fact(tcp-network)

   # Claim returns a specific value of a claim within a current fact
   baz: claim(port)

Fact Functions
^^^^^^^^^^^^^^

Fact functions are just traversal mechanism to fetch the value of an entity by another consumer.

``fact(value | @)``

  Function ``fact`` can accept the following parameters:

    - A type of ``String``, which is an ID of a specific fact. This would be a static call.
    - An ``@`` symbol, which refers to a current fact. This would be a dynamic call.

  The function will return the entire structure of a fact data as it is represented in the Model.
  It is sometimes useful if a module already "understands" the structure "as is" without adjusting it
  within an **action** consumer.


``claim(value)``

  Function ``claim`` accepts a type ``String`` with the ID of that claim.

  The function returns a defined value of that claim.

``static()``

  Function ``static`` accepts a type ``String`` with the whole absolute path with the ID of the claim.

  A path has ``.`` dot-notation, e.g. ``foo.bar.baz`` where ``baz`` is the final ID.
