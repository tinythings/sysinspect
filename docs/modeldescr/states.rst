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

States
======

.. note::

    This document describes the concept of entity states.

In a system architecture, an :bi:`entity state` refers to the current condition
or status of any component *(referred to as an entity)* within the system,
determined by a specific set of attributes or behaviors that reflect its
operational context at a given moment in time. The state of an entity captures
its functional status, configuration, and interaction capabilities relative to
its environment. For example, an entity like a car engine may have states such
as running or standby, while a network connection could be in states such as
active or disconnected. The entity state is dynamic and changes based on the
entity's interactions, inputs, or internal conditions, facilitating the
management, monitoring, and control of the system as a whole.

Synopsis
--------

States are basically a free form ID strings, put where they are supported: in actions,
entities, constraints and relations. For more details, refer to the documentation
of the specific area.

.. important::

    States must have the same ID across the Model Description so then they can match,
    once selected.

Usage
-----

:bi:`Sysinspect` can accept several states, excluding all others. If no keyword ``--states``
is passed, then default state is selected:

.. code-block:: text

    syspect --model ./my_model --states=online,active
