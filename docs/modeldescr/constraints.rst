Constraints
===========

.. note::

    This document explains how constraints work and what they are for.

Constraints are declarative logic carriers. They provide clear rules what to *consume* within
a specific fact.

.. important::

    The following rules are applied to a constraint:

    - It provides declarative logic for modules those are consuming a specific fact
    - It operates on actions, using entities's data

Synopsis
--------

Constraints has the following syntax:

.. code-block:: text

    constraints:
      <action id>:
        descr: description
        entities:
          - [$]
          - [entity id]
        <condition> (all|any|none):
          <state> (label|$):
              - fact: <value>
              - <expr> (equals, less, more, matches (regex), contains, starts, ends): <value|claim()>

Collection of constraints is under ``constraints`` section. Constraint is always attahed to an action
by the same Id. Same as an action, it is binding to a list of entities. For example, an action can
bind to many entities, but its constraint can be limited to only few, etc. Each constraint has its own
set of expressions.

Entity Binding
--------------

Entities are bound by the section ``entities``. The following rules apply:

1. A list of entities (at least one) means that the constraint will work _only_ on these entities.
2. If a list has only one element and it is ``$`` (all), then the constraint will catch all entities.
3. If a list contains some entity Ids *as well as* ``$`` (all), then the constraint will catch all entitles, *except* those that are listed in the ``entities`` section.

Expressions
-----------

Expressions are defined in the ``<state>`` block. Either it is ``$`` (any) or a specific state.

Conditions
^^^^^^^^^^

A condition has the following syntax:

.. code-block:: text

  <condition>:
    <state>:
      - fact: <value>
      - <operator>: <value> | claim()

These are currently supported conditions:

``any``

    This expression is a logical ``OR``. It means that at least one of the items must be ``true``.

``all``

    This expression is a logical ``AND``. It means that all of the items must be ``true``.

``none``

    Essentially, this is an inversion to ``all``.


The following operators are supported:

- ``equals`` — compare fact and the value on all types for their equality.
- ``less`` — compare fact and the value of int/float types if the fact is less then the value.
- ``more`` — compare fact and the value of int/float types if the fact is more then the value.
- ``matches`` (regex) — match a string value with the regular expression.
- ``contains`` — check if a value contains the defined part of a string.
- ``starts`` — check if a value starts with the defined part of a string.
- ``ends`` — check if a value ends with the defined part of a string.

Each operator can contain a static value or a dynamically call a current claim via ``claim()`` function.

Example
^^^^^^^

.. code-block:: yaml

    actions:
      # NOTE: Same id as in constraints
      net-addr-verification:
        descr: Check addresses
        module: sys.net
        bind:
          - addresses
        args:
          - iface: "claim(if)"
          - inet: "claim(inet)"

    entities:
      # NOTE: An id of an entity, referred by a constraint below
      addresses:
        facts:
          $:
            - wifi:
                if: wlp0s20f3
                inet: 192.168.2.151/24

            - virtual-main:
                if: virbr0
                inet: 192.168.122.1/24

            - virtual-secondary:
                if: virbr1
                inet: 192.168.100.1/24

    constraints:
      # Corresponds to the same action Id
      net-addr-verification:
        descr: Interfaces have assigned addresses
        all:
          wifi:
            - fact: if-up.wlp0s20f3.IPv4
            - equals: claim(wifi.inet)

        any:
          $:
            - fact: if-up.virbr0.IPv4
              equals: claim(virtual-main.inet)

            - fact: if-up.virbr1.IPv4
              equals: claim(virtual-main.inet)

Origin of "fact"
^^^^^^^^^^^^^^^^

One might ask where is the ``fact`` value comes from and what is this namespace
in the example above, like ``if-up.virbr0.IPv4``?

This is the arbitrary data in the plugin output. Since a plugin can return literally
*anything possible*, the navigation over the structure is entirely on user. The
namespace is basically keys of nested maps. So the ``if-up.virbr0.IPv4`` would correspond
to this JSON data structure:

.. code-block:: json

    {
      "if-up": {
        "virbr0": {
          "IPv4": "192.168.0.2",
        }
      },
    }

However, sometimes structure can have a bit different result (i.e. each final key/value is
an element in the array):

.. code-block:: json

    {
      "if-up": {
        "virbr0": [
          {"IPv4": "192.168.0.2",},
        ]
      },
    }

If the same key/value happens twice or more, first in the line wins.

.. note::

  The data navigation is still under development and is subject to change.