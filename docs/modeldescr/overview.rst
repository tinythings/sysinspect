Model Description
=================

.. note::
   Document explains the Model Definition and its components.

.. _model_description:

The Model is essentially a configuration of a system. It serves two purposes:

- An assertion verification about system integrity (information only)
- A state enforcement as a configuration management system does


The Aodel is written in YAML format, and it is following a specific expression schema and logic.

This document explains each part of the Model Description.

.. toctree::
   :maxdepth: 1

   layout
   entities
   actions
   constraints
   relations
   states
   events
   functions
   inheritance
   telemetry

Model Description is written in YAML format, using **declarative method**. Declarative approach
is considered better than imperative approach for several reasons.

**Declarative Approach**

   It allows to define the desired end state of the system without specifying the step-by-step
   instructions to achieve it. With declarative configuration, one can abstract away the low-level
   details and apply the same configuration across many systems or environments. This allows to reuse
   the same Model Description, only inheriting specific parts to be changed.

   Declarative configurations are usually shorter and easier to maintain because they allow to focus
   only on the final result, instead on the individual steps. Changes are more intuitive and involve
   only *describing* how the system should look after the change.

**Imperative Approach**

   It requires describe the steps explicitly in order to get the system to the desired state. Imperative
   approach requires writing detailed code **for each environment**. This usually leads to a complex,
   difficult-to-manage configurations when scaling across many systems. Imperative instructions tend
   to grow more complex as more steps are added, making them very hard to debug and maintain over time.

The declarative approach is much easier to comprehend and use, because it focuses on what the
system should do, rather than how to achieve it. In this way more readable and maintainable
configurations are achieved. Maintenance is easier because the configurations remain simple and predictable.

Recent versions of the model description also allow the author to declare an explicit public
interface for the model. This is useful when a model contains helper entities, gating actions,
or intermediate implementation details that are necessary for evaluation but are not meant to be
 presented as first-class query targets to operators. In that situation the model may declare an
``interface`` section and list only those entrypoints that are intended to be public. If the
section is omitted, the legacy behaviour remains in effect and every inferred entrypoint is
considered public.

.. important::

   In a nutshell, declarative configurations are easier for teams to understand, update, and share
   for collaboration.
