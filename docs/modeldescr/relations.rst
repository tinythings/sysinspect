Relations
=========

.. note::
   This document describes relations as a map of entities within specific architecture.

Synopsis
--------

.. important::

    The following rules are applied to a relation:

    - It defines edge relation between two or more entities within the specific system
    - It is only a transport between the claims of an entity

Relations form a dependency graph, very similar to a typical package manager in a regular Linux
distribution, such as RPM or Dpkg. However, the main difference between a map of package
relations for their dependencies and a system architecture map is that packages are static
and unchanging, with only one map in the form of a simple graph. In contrast, an architecture
can have a multi-layered graph that represents different relation types, depending on the
states of entities or the entire system's state or mode.

A working live system can have various settings, modes, component configurations, time usages,
and so on. For example, a car's engine requires a small starter motor. This is one state of
the system, where the starter is required. However, once the main engine is running, the system
is in a different state, where the starter is disengaged and no longer needed to maintain
this state.

Relation syntax as follows:

.. code-block:: text

   relations:
      <entity-id>:
        $|<state-id>:
          requires:
            - <entity-id>
          consists:
            - <entity-id>
          conflicts:
            - <entity-id>

``$``

  A default state.

``<state-id>``

  An ID (label) of a specific state, with all relevant conditions to that state.

``requires``

   A condition section containing other entities that are required to maintain the specific state
   of an entity.

``consists``

   A condition section containing **other additional** entities that form and complete the current entity.

``conflicts``

   Entities that are in conflict with the current state. For example, an engine starter should no
   longer be running in a car if the main engine is already running.


The following example shows the relation under two system states:

.. code-block:: yaml

    relations:
      car-engine:

        # Standby temporary engine state
        standby:
          requires:
            - starter
            - battery
            - fuel-tank

        # Running engine state
        running:
          requires:
            - fuel-tank
          conflicts:
            - starter

        # Cold engine state (turned off)
        $:
          consists:
            - starter
            - fuel-tank
            - battery
