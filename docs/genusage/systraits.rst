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

Traits are essentially static attributes of a minion. They can be a literally anything
in a form of key/value. There are different kinds of traits:

**Common**

   Common traits are given to each minion automatically. They are typical system
   information and anything else that can be commonly fetched out of the "box": OS info,
   kernel version, memory size, network settings, hostnames, machine Id etc.

**Custom**

   Custom traits are static data that set explicity onto a minion. Any data in
   key/value form. They are usually various labels, rack number, physical floor,
   Asset Tag, serial number etc.

**Dynamic**

   Dynamic traits are custom functions, where data obtained by relevant modules.
   essentially, they are just like normal modules, except the resulting data is stored as
   a criterion by which a specific minion is targeted. For example, *"memory less than X"*,
   or *"runs process Y"* etc.

Listing Traits
--------------

To list minion's traits, is enough to target a minion by its Id or hostname:

.. code-block:: bash

    $ sysinspect --minions
    ...

    $ sysinspect --info <minion-id-or-hostname>

Using Traits in a Model
-----------------------

Using traits in a model is described in :ref:`splitting_claims` chapter of the :ref:`distributed_model` document.

Static Minion Traits
--------------------

Traits can be also custom static data, which is placed in a minion configuration. Traits are just
YAML files with key/value format, placed in ``$SYSINSPECT/traits`` directory of a minion. The naming
of those files is not important, they will be anyway merged into one tree. Important is to ensure
that trait keys do not repeat, so they do not overwrite each other. The ``$SYSINSPECT`` directory
is ``/etc/sysinspect`` by default or is defined in the minion configuration.

Example of a trait file:

.. code-block:: yaml
    :caption: File: ``/etc/sysinspect/traits/example.trait``

    traits:
      name: Fred

From now on, the minion can be targeded by the trait ``name``:

.. code-block:: bash
    :caption: Targeting a minion by a custom trait

    sysinspect "my_model/my_entity name:Fred"

.. code-block::

Populating Static Traits
------------------------

Populating traits is done in two steps:

1. Writing a specific static trait in a trait description
2. Populating the trait description to all targeted minions

Synopsis of a trait description as follows:

.. code-block:: text
    :caption: Synopsis

    <query>:
      [machine-id]:
        - [list]
      [hostname]:
        - [list]
      [traits]:
        [key]: [value]
    <traits>:
      [key]: [value]

    # Only for dynamic traits (functions)
    [functions]:
      - [list]

For example, to make an alias for all Ubuntu running machines, the following valid trait description:

.. code-block:: yaml
    :caption: An alias to a system trait

    # This is to select what minions should have
    # the following traits assigned
    query:
      traits:
        - system.os.kernel.version: 6.*

    # Actual traits to be assigned
    traits:
      kernel: six

Now it is possible to call all minions with any kernel of major version 6 like so:

.. code-block:: bash
    :caption: Target minions by own alias

    sysinspect "my_model/my_entity kernel:six"

The section ``functions`` is used for the dynamic traits, described below.

Dynamic Traits
--------------

Dynamic traits are functions that are doing something on the machine. Since those functions
are standalone executables, they do not accept any parameters. Functions are the same modules
like any other modules and using the same protocol with the JSON format. The difference is that
the module should return key/value structure. For example:

.. code-block:: json

    {
        "key": "value",
    }

Example of using a custom module:

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

Any modules that return non-zero return like system error more than ``1`` is simply ignored
and error is logged.

Populating Dynamic Traits
-------------------------

To populate dynamic trait there are three steps for this:

1. Writing a specific trait in a Trait Description
2. Placing the trait module to the file server so the minions can download it
3. Populating the Trait Description to all targeted minions

To write a specific trait in a Trait Description, the ``functions`` section must be specified.
Example:

.. code-block:: yaml

    functions:
      # Specify a relative path on the fileserver
      - /functions/my_trait.sh

The script ``my_trait.sh`` will be copied to ``$SYSINSPECT/functions``. When the minion starts,
it will execute each function in alphabetical oder, read the JSON output and merge the result
into the common traits tree. Then the traits tree will be synchronised with the Master.

.. important::

    While function traits are dynamic, they are still should be treated as static data.

While function sounds dynamic, the trait is still an attribute :bi:`by which` a minion is queried.
This means if the attribute will be different at every minion startup, it might be useless
to target a minion by such attribute, unless it is matching to some regular expression. There
might be a rare use cases, such as *"select minion or not, depending on its mood"* (because the
function returns every time a different value), but generally this sort of dynamism is nearly
outside of the scope of traits system.