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

.. _checkbook_tutorial:

Checkbook Tutorial
===================

.. note::
    This tutorial must walk you through a minimal Checkbook definition in Sysinspect model.
    It is more of a "Hello, World!" for Checkbook than a real-life example, but it should be enough to get you started.

    Topics covered in this tutorial:

    1. Checkbook definition
    2. What "relations" are
    3. Assertion (constraints) definition
    4. Action chain definition
    5. Running the Checkbook locally

Before You Start
================

Sysinspect keeps the philosophy of UNIX: a chain of small, composable tools. It doesn't try to describe the whole Universe
in its own language, but rather provides a powerful framework to define your own models and tools. Therefore, you should not expect
a "quick-n-dirty" easy stuff — this is what other Configuration Management systems are for. 😊

A Checkbook is what one would call a "playbook" in Ansible or a "formula" in SaltStack. It is a collection of "relations"
(facts, assertions, actions) aligned in a logical pipeline flow, that are evaluated and executed in a certain order.


Model Definition
================

Model typically can be either one file ``module.cfg`` as an analogy to ``index.html`` in a website, or a directory with multiple files.
In the latter case, the main file is still ``module.cfg`` in the root of the directory tree. In this case, the whole model will be
automatically merget together.

Main file must have ``name``, ``version`` and ``description`` fields, but the rest is up to you. In this tutorial we will define
everything in one file.

.. code-block:: yaml

    name: Checkbook Tutorial
    version: "0.1"
    description: |
      Example how to make checkbooks and relations
    maintainer: Mykola Mustermann <mymuster@beispiel.de>

Checkbook Definition
--------------------

# 1. Define a checkbook
#
#    Call is by a label "os-check"
#    This label is basically a collection of
#    various entities that needs to be fired in the order
#    to call a chain of actions per each entity

Checkbook has quite a simplle syntax: a unique ID (``os-check`` in this case) and a list of relations to be called.
Relations are defined in the next section. You can refer to the Model Definition documentation for more details on the syntax.
Relations are actually the elements that consists of more actions, assertions etc. So if you call a checkbook, SysInspect will
execute it one after another in serial way and in the exact order.

.. code-block:: yaml

    checkbook:
      os-check:
        - os-info
        - net-info

Relation Definition
-------------------

.. warning::

    The syntax of relations is still in the early stages of development, so it is subject to change in the future.

Even though relation syntax may still change a bit or more features added, the basic idea is already there. In this example
we define two relations: ``os-info`` and ``net-info``. Each relation is a collection of entities that needs to be called to execute the relation.
Entities are defined in the next section.

Here we will inquire ``machine-id``, ``os-version`` and ``os-packages`` for the first relation, and ``routing`` for the second one. So when we call
the checkbook, it will execute all these actions in the order defined in the relations.

.. code-block:: yaml

    relations:
      # Common info returns a set of data about the OS
      # In this case machine-id, os version and amount of installed packages
      os-info:
        $:
          requires:
            - general-info
            - packages-info

      net-info:
        $:
          requires:
            - routing-info

Entities Definition
-------------------

In the relation definition above, we had ``general-info``, ``packages-info`` and ``routing-info`` as the required entities.
Now we need to define these entities. To make this example more complete, entity definitions can also contain facts they carry.
Let's update ``routing-info`` entity to contain a fact with some relevant information:

.. code-block:: yaml

    entities:
      general-info:
        description: General information about the OS
      packages-info:
        description: Information about installed packages
      routing-info:
        description: Routing information
        claims:
          $:
            - addresses:
                subnet: "192.168.122.0/24"

In this case it is expected that once ``routing-info`` is called, it will may be able to inquire about the routing information
and claim that there is a subnet ``192.168.122.0/24``.

.. note::
    SysInspect assertions/inquiries are **fully separated**, are optional and decoupled from the action execution flow.
    You can assert anything you want, or omit the whole assertion part.

    Also note, that results of these assertions are also completely decoupled and supposed to be handled separately
    by event handlers. An assertion is just that: an event.


Actions Definition
------------------

Let's define some simple actions. In this case we will be using ``sys.run`` module to just execute some existing commands on the
target and simply get the text output. The result of these actions and the way we obtain the data is not that important here,
as this is not the topic of this particular tutorial.

These actions will be called by the relations defined above.


.. code-block:: yaml

    actions:
      # This action will just display the content of /etc/machine-id file
      machine-id:
        descr: Display /etc/machine-id
        module: sys.run
        bind:
          - general-info
        state:
          $:
            args:
              cmd: "cat /etc/machine-id"

      # This action will display the OS version by parsing /etc/os-release file
      os-version:
        descr: Display OS version
        module: sys.run
        bind:
          - general-info
        state:
          $:
            args:
              cmd: "sed -n s/^VERSION_ID=//p /etc/os-release"

      # This action will get package stats
      os-packages:
        descr: Get package stats
        module: sys.run
        bind:
          - packages-info
        state:
          $:
            args:
              cmd: "apt-cache stats"

      # This action will display the routing information by executing "ip route" command
      routing:
        descr: Display routing information
        module: sys.run
        bind:
          - routing-info
        state:
          $:
            args:
              cmd: "ip route"

Here we define the last action for the second relation. Note that it has a constraint defined in the
next section, so it will be executed only if the previous action ``routing`` is successful.

.. code-block:: yaml

  actions:
    # This action will display the network addresses by executing "ip addr" command
    addr:
      if-true:
        - routing
      descr: Display network addresses
      module: sys.run
      bind:
        - routing-info
      state:
        $:
          args:
            cmd: "ip addr"


Asserts Definition
------------------

Assertions are defined in the ``constraints`` section. They are basically a set of conditions that
needs to be satisfied for the action to be executed. As this tutorial is not focused on assertion
details, we defined one only to demonstrate the concept. In this case, we want to check if the output
of the ``routing`` action contains the subnet ``192.168.122.0/24``, and fail if it doesn't, effectively
preventing the next action ``addr`` from being executed.

.. code-block:: yaml

  constraints:
    routing:
      descr: Check for virtual subnet 192.168.122.0/24 present
      entities:
        - routing-info
      all:
        $:
          - fact: stdout
            contains: "192.168.122.0/24"

Events Definition
-----------------

Everything in Sysinspect is an event, and you can handle these events in any way you want. For example,
you can log them to the console, or send them to some external system for further processing. In this
section we define two event handlers: one for logging all events to the console, and another one for
logging only the results of assertions (constraints) to a separate file.

.. warning::

  **Events section** will be extended from the scope of just model definition, to share the
  namespace with **Sensors section** which will be introduced in the next major release.


  While the current syntax itself won't change, model events will be overridden by the events of sensors,
  so you will need to make sure your model events do not clash with the sensor events.

  Please refer to the documentation of the next major release for more details on this topic.

.. code-block:: yaml

  events:
    $/$/$/$:
      handlers:
        - console-logger
        - outcome-logger

      console-logger:
        concise: false
        prefix: "Checkbook Tutorial"

      outcome-logger:
        prefix: Constraints
