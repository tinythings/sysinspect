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

.. _cfgmgmt_tutorial:

General Configuration Management
================================

.. note::
   This documentation walks you through a minimal Configuration Management
   routine to use Sysinspect as a traditional Configuration Management system.

   **In just just eight (8) crazy steps you can write your "Hello, World"!**

   Should be fun...

Before You Start
----------------

A traditional Configuration Management operations is an :bi:`optional` feature
in Sysinspect. Therefore, do not expect it as super-easy as in Ansible or SaltStack. 😊

Nevertheless, this is the same Configuration Management as everywhere else!
Follow this tutorial to understand how it works.

.. important::

    Don't expect it to begin with as super-easy as in Ansible or SaltStack.
    You would still need a model for all that!


Model Definition
----------------

.. warning::
    This tutorial is written in a bit different form than any other tutorials and
    may contain puns that a soft romantic soul might find weird. You've been warned.

Step 1: Initialise Your Model
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

Before you complain how difficult your life is, please remember: Sysinspect is
a model-based system. We don't do "quick-n-dirty" easy stuff — this is what
other Configuration Management systems are for. 😊

So go on. Create a model index config somewhere, say ``/your/models/cfgmgmt/model.cfg``
and add the following content:

.. code-block:: yaml

    name: Configuration management example
    version: "0.1"
    description: |
      Configuration management example model

    maintainer: Your Name <rants@javasucks.com>

Save it and let's move to the next step.

Step 2: Define Checkbook
^^^^^^^^^^^^^^^^^^^^^^^^

While Sysinspect can allow you to call only a specific part of the model,
still it is a good idea to first define a checkbook. It is like a Playbook
in Ansible: you define the entire block to run. Yes, in the same ``model.cfg``, yes!

So let's do this:

.. code-block:: yaml

    checkbook:

Well yes, you don't need it right now. But the entry is still required.

Step 3: Define a Relation
^^^^^^^^^^^^^^^^^^^^^^^^^

In best government traditions, Sysinspect wants to know who belongs to who.
So let's do it:

.. code-block::  yaml

    relations:

But who needs extra relations, right?.. Checkbook is empty, so are the relations.

Step 4: Describe Entities
^^^^^^^^^^^^^^^^^^^^^^^^^

You are going to do something with your system, right? I don't know what it is,
you tell me! Or, rather don't, I won't be there to fix your broken machine...
Still, as we only want to let you understand how all that works in principle,
we will do some file operations, using some copying or creating absolutely useless
files in your ``/tmp`` directory.

So let's do it. We need to define an entity that would be responsible for
file operations:

.. code-block:: yaml

    entities:
      file-ops:
        claims:
          $:
            - netconfig:
                name: /tmp/networks

So what we did here, is that we defined some :bi:`claims` that we then later
can use. Think of it like we defined constants. In our case, we said that
in all possible states (``$``) the ``netconfig`` will have always ``name``
set to the ``/tmp/networks`` path. We can reuse that later on, if we want to.
It is just that: a variable ``name`` has value ``/tmp/networks``.

But we do not *have* to and we can hard-code stuff all around the place, making
it hard to maintain. In this case you can happily leave it as useless
as the other two:

.. code-block:: yaml

    entities:

That's all.

Step 5: Define an Action
^^^^^^^^^^^^^^^^^^^^^^^^

We want to kick that finally, aren't we? Here you go:

.. code-block:: yaml

    actions:
      copy-netconfig:
        descr: Copy network configuration to /tmp
        module: fs.file
        bind:
          - file-ops

        state:
          $:
            opts:
              - fill
            args:
              name: /tmp/networks.cfg

We've just created an action, called *"copy-netconfig"* and it is using ``fs.file`` module.
As we only care to call it at all, regardless of a specific state *(and a traditional Configuration Management
doesn't really have any)*, we use ``$`` for that. What we want to do here, is to fill a
file with the content at ``/tmp/networks.cfg``, served on the master's HTTP server.

If we run all that, the file will be created. But wait, we didn't finished it just yet!
This is Sysinspect, which carefully inspects everything it is touching. Therefore we need to know
if the result of our actions was correct, as well as we have to send that result to the
event engine, so the appropriate metrics are generated.

Step 6: Define an Assertion
^^^^^^^^^^^^^^^^^^^^^^^^^^^

You want it checked, aren't you? Otherwise you will be just seeing "oh, it worked" or "oh, it didn't".
So, here you go:

.. code-block:: yaml

    constraints:
      copy-netconfig:
        descr: Check network confinguration had happened
        entities:
          - file-ops

        all:
          $:
            - fact: changed
              equals: true

Our ``fs.file`` module returns some data. And this is called as :bi:`facts`. So we grab those facts
*(depends on a module)* and check if they are what do we expect.

Now, every time it will create a file, it will say *"Great, I did it!"* and things will be "green".
And as long as you will start it again, it will say *"Hey, it is there already!"*. Idempotence, you know?..

No, please don't make it empty useless. You definitely need an assertion here.

Step 7: Define Events
^^^^^^^^^^^^^^^^^^^^^

Once data is collected, we need to do with that something, isn't it? Otherwise, what's the point...
Let's do it:

.. code-block:: yaml

    events:
      $/$/$/$:
        handler:
          - console-logger
          - outcome-logger

        console-logger:
          concise: false
          prefix: CfgMgmt

        outcome-logger:
          prefix: CfgMgmt

Here we are routing literally everything through two event handlers: *console-logger* and *outcome-logger*.

Step 8: Enable Model in Master Config
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

You are probably already have that config and your master is looking into the directory where each
subdirectory is another model. So if you just created a new one, it needs to be first enabled in the
configuration file. Remember that path over there at the beginning of this document? So your
master is serving all models by the path ``/your/models`` and then ``cfgmgmt`` is the subdirectory
where ``model.cfg`` is placed with all that content we've just made:

.. code-block:: yaml

    fileserver.models:
      - cfgmgmt

Applying State
--------------

OK, so now start master and minion and see what happens *(assuming the installation is correct and the
modules are all there)*:

.. code-block:: shell

    $ sysinspect cfgmgmt/file-ops '*'

In your output log you should see something like that:

.. code-block:: text

    [11/02/2025 20:07:42] - INFO: Starting sysinspect runner
    [11/02/2025 20:07:42] - INFO: CfgMgmt - file-ops/copy-netconfig - File /tmp/networks.cfg created
    [11/02/2025 20:07:42] - INFO: CfgMgmt - file-ops/copy-netconfig - Other data:
    Key        Value
    changed  true

    [11/02/2025 20:07:42] - INFO: CfgMgmt - copy-netconfig assertions passed

Call it again, why not? Now it should complain at your irresponsible actions:

.. code-block:: text

    [11/02/2025 20:29:21] - INFO: Starting sysinspect runner
    [11/02/2025 20:29:21] - INFO: CfgMgmt - file-ops/copy-netconfig - Touch error: File /tmp/networks.cfg already exists
    [11/02/2025 20:29:21] - INFO: CfgMgmt - file-ops/copy-netconfig - Other data:
    Key        Value
    changed  false

    [11/02/2025 20:29:21] - ERROR: CfgMgmt - Networks config should be copied again: changed fact fails as false

See? It worked, so don't try to turn it OFF and ON again, even though you can...

Final Notes
------------

I usually refer to a bottle of beer. 😊 You can keep entertaining yourself removing that
new file in your ``/tmp`` directory and keep repeating calling your minimal Configuration Management
model.

**Congratulations, this tutorial is over!**

If You Are Still Curious
------------------------

But you are curious how to avoid those hardcodings and what it is all about it, right?
So let's expand our lonely and empty ``entities:`` to a bit more information as it is in
those previous step:

.. code-block:: yaml

    entities:
      file-ops:
        claims:
          $:
            - netconfig:
                name: /tmp/networks

Now, in the :bi:`actions` section we've hard-coded that path. But we can rather invoke it
instead, using function *claim(...)*. Let's do that and change in the ``actions:`` part
the args section to this:

.. code-block:: yaml

    actions:
      ...
        args:
          name: "claim(netconfig.name)"

The function will go and resolve the value of that ugly dot-notated path within the current
state. In our case we use "any state" or ``$`` — a dollar sign, which is the same as ``*``
asterisk in a typical Unix clobbing. Now you are slowly getting the idea: if you would use
some *other* state, then you could use another claim value, and therefore re-route your
results to a different events as well as use even different options to the same module.
I know what you are thinking, but don't do it right now.

Yes, that looks a bit complicated. But that's all for now! Go finish your beer
and have a nice evening. 😊
