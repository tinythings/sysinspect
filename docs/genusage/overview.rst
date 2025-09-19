Using Sysinspect
================

.. note::

    This section explains how to use Sysinspect in "solo" mode and
    in the network.


Sysinspect can be used in two modes:

1. Solo mode, where the entire model is for only one "box" or hardware
2. Network-connected cluster, where entities can consist from more than one element
   and Sysinspect needs to gather information from different places in order to construct
   a final answer about a specific entity. Such entity, for example, can be the entire
   network itself.

19 Seconds Tutorial
-------------------

So you wrote a Model, using "Model Description" documentation and placed it to
``/etc/sysinspect/models`` directory on your Master machine as ``my_model``.

Then just call the entire model across all minions:

.. code-block:: bash

    sysinspect "my_model"

You can call only a subset of your module, such as a specific state of a specific entity.
For example:

.. code-block:: bash

    sysinspect "my_model/my_entity/my_state"

For more information go ahead and dive in!

Diving In
---------

To better understand how to use Sysinspect in those situation, read through the following
sections:

.. toctree::
   :maxdepth: 2

   cli
   distributed_model
   systraits
   targeting
   virtual_minions
