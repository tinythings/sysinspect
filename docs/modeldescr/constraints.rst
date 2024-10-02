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
      - <label id>:
        descr: description
        expr:
          <action id>:
            <operator>:
              - condition
              - condition-1
          $:

Collection of constraints is under ``constraints`` section, assigning an ID per a constraint.
Each constraint has its own set of expressions. Constraint works on all entities within an action.
Example:

.. code-block:: yaml

    action:
      bind:
        - foo
        - bar

    constraint:
      ...

In the example above means that when action will process ``foo`` and ``bar`` entities, constraint
will be applied to each of them. Matchers by entity ID allows to process which entity has a different
logic applied.

Expressions
-----------

Expressions are defined in ``expr`` block. This block has list type.

Conditions
^^^^^^^^^^

``any : list``

    This expression is a logical ``OR``. It means that at least one of the items must be ``true``.

``all : list``

    This expression is a logical ``AND``. It means that all of the items must be ``true``.

``present : list``

    Block any facts that do not specified claims.

``absent : list``

    Block any facts that do have specified claims.

Entity Matching
^^^^^^^^^^^^^^^

``$``

    This is like globbing ``*`` (all others). It means that any other items that wasn't yet
    processed go here. It is used in case when an action has multiple constraint

Example:

.. code-block:: yaml

    expr:
      # Only entity with the ID "some_entity" is processed.
      some_entity:
        ...

      # appliccable to all other entities
      $:
        ...


.. important::

    Constraints are bound to actions by having the same ID.

Consider the following example. Defined constraint says that any of the facts over
current action must pass:

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
      - addresses:
          facts:
            wifi:
              if: wlp0s20f3
              inet: 192.168.2.151/24

            virtual-main:
              if: virbr0
              inet: 192.168.122.1/24

            virtual-secondary:
              if: virbr1
              inet: 192.168.100.1/24

    constraints:
      # NOTE: Same id as actions
      net-addr-verification:
        descr: Interfaces have assigned addresses
        expr:
          # Only entity with the id "addresses" is processed
          - addresses:
              # The whole fact is processed if it has "wifi" claim
              present:
                - wifi
              any:
                - virtual-main
                - virtual-secondary
              all:
                - wifi
