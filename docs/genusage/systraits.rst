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

.. _systraits:

System Traits
=============

.. note::

    Definition and description of system traits and their purpose.

Traits are essentially static attributes of a minion. They can be almost anything
in a key/value form. There are different kinds of traits:

**Common**

   Common traits are given to each minion automatically. They are typical system
   information and anything else that can be commonly fetched out of the "box": OS info,
   kernel version, memory size, network settings, hostnames, machine Id etc.

**Custom**

   Custom traits are static data that are set explicitly onto a minion. Any data in
   key/value form. They are usually various labels, rack number, physical floor,
   Asset Tag, serial number etc.

**Dynamic**

   Dynamic traits are custom functions where data is obtained by relevant modules.
   Essentially, they are just like normal modules, except the resulting data is stored as
   a criterion by which a specific minion is targeted. For example, *"memory less than X"*,
   or *"runs process Y"* etc.

Using Traits in a Model
-----------------------

Using traits in a model is described in :ref:`splitting_claims` chapter of the :ref:`distributed_model` document.

Static Minion Traits
--------------------

Traits can be also custom static data, which is placed in a minion configuration. Traits are just
YAML files with key/value format, placed in ``$SYSINSPECT/traits`` directory of a minion. Files
ending in ``.cfg`` are loaded and merged into one tree. The ``$SYSINSPECT`` directory
is ``/etc/sysinspect`` by default or is defined in the minion configuration.

Load order is:

1. discovered built-in traits
2. local ``*.cfg`` files in alphabetical order, except ``master.cfg``
3. trait functions from ``$SYSINSPECT/functions``
4. ``master.cfg`` last, overriding all previous values

Example of a trait file:

.. code-block:: yaml
    :caption: File: ``/etc/sysinspect/traits/example.cfg``

    name: Fred
    rack: A3

From now on, the minion can be targeted by the trait ``name``:

.. code-block:: bash
    :caption: Targeting a minion by a custom trait

    sysinspect "my_model/my_entity" "*" --traits "name:Fred"

Populating Static Traits
------------------------

Local static traits are simply written into separate ``*.cfg`` files by the
operator or provisioning system.

Master-managed static traits use a reserved file:

.. code-block:: text

    $SYSINSPECT/traits/master.cfg

This file is created automatically by the minion and is reserved for updates
coming from the master. It should not be edited manually.

The ``sysinspect traits`` command updates only this file:

.. code-block:: bash

    sysinspect traits --set "foo:bar" "web*"
    sysinspect traits --unset "foo,baz" "web*"
    sysinspect traits --reset --id 30006546535e428aba0a0caa6712e225

After such update the minion immediately sends refreshed traits back to the
master. Global ``sysinspect --sync`` also refreshes traits.

Deployment profile assignment also uses this same mechanism. For example:

.. code-block:: bash

    sysinspect profile --tag "tiny-lua" --query "pi*"
    sysinspect profile --untag "tiny-lua" --id 30006546535e428aba0a0caa6712e225

This updates the master-managed ``minion.profile`` trait on the targeted
minions.

If ``minion.profile`` is not set, the minion falls back to the
``default`` profile during sync.

``default`` is fallback-only. Once one or more real profiles are assigned,
``default`` is not kept alongside them as a stored assignment.

Dynamic Traits
--------------

Function-based traits are standalone executables placed into
``$SYSINSPECT/functions``. Since those functions are standalone executables,
they do not accept any parameters. They use the same general JSON return
shape as other modules, except that the output is merged into the minion's
trait tree.

The module should return a key/value structure. For example:

.. code-block:: json

    {
        "key": "value",
    }

Example of using a custom function:

.. code-block:: bash
    :caption: File: ``my_trait.sh``

    #!/usr/bin/bash
    kernel=$(uname -r)
    echo $(printf '{"kernel.release": "%s"}' $kernel)

The output of this script is a JSON key/value structure:

.. code-block:: json
    :caption: Example output

    {
        "kernel.release": "5.19.0-50-generic"
    }

The function module must be portable, i.e. Minion has no responsibility to ensuring if the
function module is actionable or not on a target system. I.e. user must ensure that the target
system where the particular minion is running, should be equipped with Bash in ``/usr/bin``
directory.

Any function that returns a non-zero result greater than ``1`` is ignored and
an error is logged.

The script ``my_trait.sh`` will be executed when traits are loaded. The minion
reads its JSON output and merges the result into the common traits tree.

.. important::

    While function traits are dynamic, they are still should be treated as static data.

While function sounds dynamic, the trait is still an attribute :bi:`by which` a minion is queried.
This means if the attribute will be different at every minion startup, it might be useless
to target a minion by such attribute, unless it is matching to some regular expression. There
might be a rare use cases, such as *"select minion or not, depending on its mood"* (because the
function returns every time a different value), but generally this sort of dynamism is nearly
outside of the scope of traits system.
