.. raw:: html

   <style type="text/css">
     span.underlined {
       text-decoration: underline;
     }
     span.bolditalic {
       font-weight: bold;
       font-style: italic;
     }

    /* target the actual Sphinx TOC container */
      .tight-toc,
      div.contents.tight-toc,
      div.topic.contents.tight-toc {
        background: #fcfcfc;
        border: none;
        border-radius: 4px;
        padding: 12px 16px;
        display: inline-block;
        min-width: 260px;
      }

      /* Sphinx uses ul.simple and also inserts p tags */
      .tight-toc ul,
      .tight-toc ul.simple {
        list-style: none !important;
        margin: 0 !important;
        padding: 0 !important;
      }

      .tight-toc li {
        margin: 0 !important;
        padding: 0 !important;
        line-height: 1.2 !important;
      }

      /* THIS is the missing piece: paragraphs inside list items */
      .tight-toc li > p,
      .tight-toc li p {
        margin: 0 !important;
        padding: 0 !important;
        line-height: 1.2 !important;
      }

      /* nested lists spacing */
      .tight-toc li > ul,
      .tight-toc li > ul.simple {
        margin: 2px 0 0 1.2em !important;
        padding-left: 0.6em !important;
      }

      /* title spacing */
      .tight-toc .topic-title {
        margin: 0 0 6px 0 !important;
        padding: 0 0 4px 0 !important;
        border-bottom: 1px solid #ddd;
        font-size: 0.8em;
        font-weight: 700;
        color: #777;
        text-transform: uppercase;
        letter-spacing: 0.08em;
      }

      .tight-toc a { text-decoration: none; }

   </style>

.. role:: u
   :class: underlined

.. role:: bi
   :class: bolditalic

.. _checkbook_tutorial:


.. contents:: Table of Contents
   :local:
   :depth: 3
   :class: tight-toc

Checkbook Tutorial
^^^^^^^^^^^^^^^^^^

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

Sysinspect keeps the philosophy of UNIX: a chain of small, composable tools. It doesn't try to describe the whole
Universe in its own language, but rather provides a powerful framework to define your own models and tools. Think of it
as a Swiss Army knife for system inspection and validation—you build exactly what you need, nothing more.

A Checkbook is what one would call a "playbook" in Ansible or a "formula" in SaltStack. It's essentially your
automation script—a collection of inspections and actions bundled together. The clever part is that Sysinspect breaks
things down into "relations" (facts, assertions, actions) that run one after another in a logical order. If you're
used to Ansible, think of it as a more granular way to organize your plays with built-in validation checks.

Requirements
============

This tutorial requires ``sys.run`` module to be available in your Sysinspect installation. Also you will need only ``sysinspect``
CLI tool to run the Checkbook locally.


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
Now we define those entities.

An *entity* is a named bucket of context. Relations pull in entities via ``requires:``, and then Sysinspect runs any
actions that are ``bind:``-attached to those entities.

Minimum viable entity definition is just a name + optional ``description``. You can also add ``claims``. Claims are
model-declared facts you expect to be true for that entity (or at least worth checking). They are not command output.
They are your own structured data that constraints and handlers can reference.

Below we extend ``routing-info`` with a claim. We declare that this host should have a route/subnet
``192.168.122.0/24`` (common for libvirt / NAT setups). Later we can verify it against the real command output.

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

What this means in plain terms:

* ``routing-info`` is the entity that groups network routing-related stuff.
* ``claims`` is where you put the "expected" values for that entity.
* The ``$`` key is the usual Sysinspect container for a list payload.
* We are storing a nested structure (``addresses`` ➡️ ``subnet``).

On its own, this claim does not run any command and does not prove anything. It is just data attached to the entity.
The actual check happens when an action (here: ``routing`` running ``ip route``) produces facts like ``stdout``, and a
constraint compares those facts to what you expect.

.. note::
  Assertions/inquiries are optional and separate from action execution.

  Actions do work and produce facts (``stdout``, ``stderr``, return code, and any module-specific fields). Constraints
  read those facts and emit pass/fail outcomes. They do not "change" the action result; they just gate follow-ups.

  Also: assertion results are handled by event handlers. A constraint outcome is an event you can log, filter, or
  route elsewhere.


Actions Definition
------------------

Now we wire up a few actions. For this tutorial we keep it straight simple on purpose: we use ``sys.run`` to just run a
shell command every time, and capture its output.

What you should know about these actions:

* They are listed under ``actions:`` section.
* The action key (for example ``machine-id``) is the action name you reference from relations/constraints.
* ``module: sys.run`` means "execute a command".
* ``bind:`` attaches the action to one or more entities. If a relation requires an entity, actions bound to
  that entity become eligible to run for that relation.
* ``state: $: args: cmd: ...`` is where the module arguments go.
* The usual outputs land in facts like ``stdout``, ``stderr``, and a return code (often ``retcode``). Those
  facts are what constraints can read later.

These actions are pulled in by the relations from the previous section (``os-info`` and ``net-info``).


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

One more action for the network relation: ``addr``. It's chained behind ``routing``.

When ``if-true:`` is listing ``routing`` action, that means Sysinspect will only run ``addr`` action after ``routing`` has completed and
successfully pass its constraints. In practice that means:

* the ``routing`` action ran and didn't error out
* if you attach any kind of assertion to ``routing`` (next section), that assertion must pass

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

Assertions live under the ``constraints`` section. Think of them as guardrails for actions: run an
action, look at what it returned, then decide if we keep going.

In this tutorial we keep it simple and define one constraint for the ``routing`` action. The idea:

* run ``routing`` (which executes ``ip route``)
* inspect its ``stdout``
* require that it mentions the subnet ``192.168.122.0/24``

If the check fails, Sysinspect marks the constraint as failed and anything chained after it (like
``addr`` which has ``if-true: - routing``) won't run. No magic: it's just a gate.

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

In this case, the assertion checks if the ``stdout`` fact (the output of the command) contains the specified subnet
within the output text.

More detail on what is happening here:

* ``constraints: routing:`` attaches this constraint to the action named ``routing``.
* ``entities:`` scopes it to the ``routing-info`` entity (so the event is tied to that entity).
* ``all:`` means every listed rule must pass.
* ``fact: stdout`` refers to the action result field called ``stdout``.
* ``contains: "192.168.122.0/24"`` is a substring match.

So if your routes don't show that subnet, the constraint fails and the next step is blocked. If the
subnet is present, the constraint passes and Sysinspect is allowed to run ``addr``.

Events Definition
-----------------

Sysinspect emits events while it runs your checkbook. Treat it like a small event bus: actions,
constraints, and other steps produce messages you can route to handlers.

Typical events you will see:

* Action lifecycle (started/finished/failed)
* Command output (``stdout``/``stderr``) and return code
* Constraint results (pass/fail + what matched or did not match)

You decide what to do with these events: print them, write them to a file, ship them to a log system,
or trigger some side-effect. In this tutorial we configure two simple handlers:

* ``console-logger``: dumps everything to the console (good for local runs and debugging)
* ``outcome-logger``: focuses on constraint outcomes (useful when you only care about assertions)

.. warning::

  In the next major release, **Events** will share a namespace with **Sensors**.

  The syntax shown here is expected to stay, but sensor-defined events may override model-defined
  events if names collide.

  Practical advice:

  * Use unique handler names (prefix them, for example ``tutorial-console-logger``)
  * Avoid generic names if you plan to mix models and sensors
  * Re-check your handler names after upgrading

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

Trying It Out
=============

Now that we have our Checkbook defined, we can run it locally using the Sysinspect CLI tool.
This is a great way to test and debug your checkbook before deploying it in a more complex environment.

Ensure you've done these steps:

1. Save the whole model definition in just one file named ``model.cfg`` under some directory, say ``./checkbook``.
2. Ensure your local machine has installed Sysinspect and the ``sys.run`` module is available as for *sysminion*.
3. Run the checkbook using the CLI:

   .. code-block:: bash

     sysinspect --model ./checkbook --labels os-check

This should execute the checkbook, run the defined actions, evaluate the constraints, and print the events to the
console via the configured handlers.

Note, that argument ``--labels`` is plural: you can specify multuple labels to run multiple checkbooks at once, comma separated.
