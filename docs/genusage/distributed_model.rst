.. raw:: html

   <style type="text/css">
     span.underlined {
       text-decoration: underline;
     }
     span.bolditalic {
       font-weight: bold;
       font-style: italic;
     }
   </style>

.. role:: u
   :class: underlined

.. role:: bi
   :class: bolditalic

.. _distributed_model:

Distributed Model
=================

Distributed model means that the entities of it might be scattered
across the system in different "boxes" or consist of different components,
those are found in different network-connected places.

Each entity keeps some claims about it. For example, a network consists of
IP addresses, MAC addresses, some ports, some protocols etc. Each of these
elements are *"building bricks"* of the whole network entity. Suppose,
we need to ensure that the network across many "boxes" contains specific
ports, protocols, addresses, routes etc. And each "box" has different configuration
for it. This means each part of the network will have a different set of claims.

For example, a router, which is :bi:`a part of the network entity`, has specific
requirements on its own inside of it, and they are claims. But they are not a
:bi:`global` claims, they are only claims that are specific to that router.
However, we cannot admit that the network is healthy if only router alone works.
We want to check many other places and settings all over the related "boxes"
and see if each box on its own has their claims admitted as actual proven facts.
Only if :bi:`all pieces comes together`, only if all checks on all boxes/places/segments
are verified and are "green", only then we can admit that the "Network Entity"
works as expected.

To achieve this, we need to distribute the model.

.. important::

    The model is actually always distributed. It is just the way how do you write it.
    However, the model is aimed to cover the entire :bi:`system`, which is not just
    one or two "boxes" or software components, processes etc.

Model Distribution Principle
----------------------------

The static model is always copied to each minion "as is". But that means that each
minion supposed to be exactly the same in order to return a success result. However,
in a complex system, some "boxes" are Database, some of them are routers, some of them
are just storage systems, some of them are running middleware/services etc. And thus
all of these "boxes" render a final heterogeneous distributed system.

How Sysinspect dissects to the targets only what's needed?

Let's take a look at the entity description:

.. code-block:: yaml

    # General section for all entities
    entities:
      router:
        claims:
          $:
            - foo: ...

          powered:
            - foobar: ...

In this case we have an entity, callsed **"router"**, which has some default common claims,
accessible by globbing ``$`` (e.g. some admin interfaces are always there, config of hypervisor
partitions etc), and it also has claims that are valid only when it is in ``powered``
mode.

.. pull-quote::

    Now, what if we have :bi:`two different routers`, and yet they have to be
    together, in order to let the whole network work as expected? How to split
    facts between them?

For this, an entity needs to match a minion by its traits, and claims are defined under
the section, where traits are filtering (or selecting) :bi:`the relevant part` of the claims
description in a declarative way.

.. _splitting_claims:

Splitting Claims
----------------

Let's continue with the same router example, but add another one. Say, the other one
is running on ARM-64 architecture instead of on Intel x86_64, has much less memory
and has a different hostname. By these traits we can distinguish those two routers,
splitting claims among them.

Here is an example:

.. code-block:: yaml

    # General section for all entities
    entities:
      router:
        # Router 1
        my-big-one:
          traits:
            system.cpu.arch: x86_64
            system.mem.total: 16GB
            net.hostname: crocodile.local
            system.os.vendor: f5

          # Specific flaims only for big, powerful "f5" Intel-based router
          claims:
            ...

        # Router 2
        my-small-one:
          traits:
            system.cpu.arch: ARM64
            system.mem.total: 4GB
            net.hostname: frog.local

          # Specific flaims only for ARM-based router
          claims:
            ...

        # Common claims for both routers
        claims:
          $:
            - foo: ...

          powered:
            - foobar: ...

The above is the entity description that is in the master Model. However,
each minion will not get the entire model, but only :bi:`a subset` of the Model,
which is relevant to only that specific minion.

.. note::

    Each minion will get only :bi:`a subset` of the Model, relevant only to
    the current minion traits or other attributes!

The mechanism works very similar to the Model Inheritance: matching section
will replace the default claims section from the section that matches the minion.
In case both sections are matching a minion, then they will be merged. If they
overlap, then first wins. Therefore it is very important to be careful to point out
the difference in traits or other attributes, ensuring the model is not overlapping
on the minion side or renders wrong.

.. important::

    It is important to use granular and detailed targeting, in order to avoid
    claims overlap between the minions, rendering false results.