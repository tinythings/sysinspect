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

Targeting Entities
==================

.. note::

    Entities are bound to the specific hardware, which is related to a specific minion.
    This document explains how to target specific minions to complete entity description.

General
-------

Sysinspect has a query mechanism where master can target remote minions by a specific
criteria. Similarly, the Model itself can be called from different "entry points".

**Checkbook**

    A Checkbook is basically a list of "entry points" to a complex trees of entities,
    essentially a group of entities that form a feature or a set of features to be checked.

**Entities**

    A regular Model entities. This type of entry is usually used to narrow down assessment
    path.


Checkbook query is using path-like tuples to target a feature (a group of entities etc)
in the following format:

.. code-block:: text
    :caption: Precise model query synopsis

    "/<model>/[entity]/[state] [traits query]"

.. code-block:: text
    :caption: Checkbook model query synopsis

    "/<model>/[entity]:[checkbook labels]"

Since there can be many models, it is essential to select one, therefore a model Id is
always required. If ``entity`` and/or ``state`` are not specified, they are defaulted to
``$`` (all).

.. code-block:: bash
    :caption: Example of Model targeting by precise query

    sysinspect "/router/network,devices/online"

In the example above, a network is verified in a router only when it supposed to be online.
Under the hood, omitted parts are translated to "all" (``$``). E.g. ``router/network`` is
translated as ``router/network/$``, or Model name alone e.g. ``router`` is translated to
``router/$/$`` etc.

Traits query is separated with a space and essentially a list of possible traits with their
values and logical operations. See :ref:`query_targeting` for more details.

.. code-block:: bash
    :caption: Example of Model targeting by checkbook labels

    sysinspect "/router:network,devices"

The example above is the same as the previous one, except it is using Checkbook. Entities
in the Checkbook are basically the top-high groups of other entities.

.. hint::

  Trailing slash in model specification path can be omitted: ``router/network/online`` would also working
  the same way.

Using Traits
------------

Every minion, running on the system can be targeted with specific criterion, describing it.
Each :bi:`minion` has a set of attributes, called :ref:`systraits`. They are used to identify
and target minions directly or from the Model.

.. warning::

    Using dynamic or static traits strongly depends on the use case of the Model. In terms of
    portability, even though static traits are "hard-coding" claims, they are stable to the
    system architecture. Likewise dynamic traits are move flexible, but they can also be more
    difficult to debug, when they clash with each other.


.. _query_targeting:

Query Targeting
---------------

Additionally, traits can be incorporated in the query. The main use of traits are
within the model, but sometimes one needs to target only a specific entity that has scope
exclusively bound to a specific minion. In the nutshell, the idea is to filter-out other
irrelevant minions, carrying *similar* entities.

Synopsis of the query is as following:

.. code-block:: text
    :caption: Query synopsis

    <trait> <op> <trait> <op> <trait>...

Query does not support grouping with `( ... )` parentheses and is read from left to right.
Example:

.. code-block:: bash

    "system.os.vendor:Debian and system.os.arch:ARM64
    or system.os.vendor:RHEL and system.os.arch:x86_64"

The expression above is telling Sysinspect to target minions, those are:

1. Running Linux Debian on ARM-64 architecture
2. Running Linux RHEL on x86_64 architecture

As it is very clear from the example above, the use of operators must be careful. Switch
of them differently will cause different results. For example:

.. code-block:: bash

    "system.os.vendor:Debian or system.os.arch:ARM64
    and system.os.vendor:RHEL or system.os.arch:x86_64"

The expression above is telling Sysinspect to target minions, those are:

1. Running Linux Debian
2. Running Linux RHEL on x86_64 architecture
3. Running on ARM-64 architecture


Distributed Entity
------------------

Since an entity can be something that is scattered across the boxes, a model needs to
self-adjust to different claims on different boxes. For example, a *"Network Entity"* can be
considered working :bi:`iff` one box e.g. has ``virbr0``, and the other one has ``wifi0``
network interfaces.

The following synopsis of the distributed entity notation in Checkbook:

.. code-block:: text

    <feature-label>:
      <group-label>: <query>

In order to achieve this, model should include or exclude "chunks" of itself on a particular
box, using some criteria, using Jinja-like templating expressions. Currently supported criteria
is all available minion traits *(static and dynamic via functions)*.

Each part of a model has exported built-in ``traits`` and it supports dot-notation, as well
as Python dictionary notation. The following example shows both available notations:

.. code-block:: jinja

    vendor: {{ traits.system.os.vendor }}
    hostname: {{ traits["net"]["hostname"] }}

The following literals in the templating system can be used:

- :bi:`boolean` ``true`` (or ``True``) and ``false`` (or ``False``)
- :bi:`integer` and :bi:`float` — just like in a regular Python
- :bi:`string` is any data surrounded with ``""`` double quotes, ``''`` single quotes or even with `````` backticks.
- :bi:`arrays` are a comma-separated list of literals and/or idents surrounded by
  square brackets ``[]``. Trailing comma allowed.

Templating supports all kind of comparisons and logic operators, those found in Python.

For example, the use case of "Backup over WiFi" would be expressed the following way:

.. code-block:: jinja

    backup_over_wifi:
    {% if traits.status.online and traits.device.freq_ghz == 5 %}
      - antennae
    {% endif %}

    {% if traits.system.os.vendor == "Debian" and traits.net.hostname == "storage.local" %}
      - raid
    {% endif %}
      - router

.. note::

    Please note, that the example above is just an example. The actual traits might vary!