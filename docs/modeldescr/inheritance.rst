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

Inheritance
===========

.. note::

    This document describes Model Description inheritance features.

Purpose
-------

The main purpose of the Model inheritance is to avoid maintenance of many very similar models,
those are nearly identical, but have some little differences. Another use case for this is to
have a common base model that would cover maximum possible system, and then inherit it with
only adjustments to it.

.. important::

    Inherited model must reside in its own isolated root directory and cannot be
    inside of a base model.

Importing Parent Model
----------------------

Inheritance is only chained. This means, a model can only inherit one parent model. Multi-inheritance
is not supported. Inheritance is started via ``inherits`` directive. Example:

.. code-block:: yaml

    name: My Great Model
    version: "0.1"
    description: |
        This is a description of this model
        that gives you more idea what it is etc.
    maintainer: John Smith <john.smith@example.com>
    inherits: /path/to/parent/model/

Parent model can also inheriting some other model etc.

.. note::

  Path usually is absolute but it can be also relative to the model location.

Inheriting Data
---------------

Data within the Model Description can be inherited in three ways:

- By extending an existing element or adding a new one
- Overriding an element
- Removing an element or its data

Adding
^^^^^^

Adding works as :bi:`merging a new element` to an existing structure.
Is as simple as that: just define a new element as it would be within
the same structure, without rewriting everything else.

For example, there are two files: ``base.cfg`` and ``add.cfg``:

.. code-block:: yaml
  :caption: Base structure

    # base.cfg
    data:
      key: value

.. code-block:: yaml
  :caption: Inherited structure

    # add.cfg
    data:
      other-key: value

The result will be as follows:

.. code-block:: yaml
  :caption: Both are merged

    data:
      key: value
      other-key: value

Removing
^^^^^^^^

To remove an element (disable what comes from a parent structure), one need to add ``(-)`` prefix.
For example, in two files ``base.cfg`` and ``add.cfg`` an element ``key`` withing the data will be
removed:

.. code-block:: yaml

    # base.cfg
    data:
      key: value

.. code-block:: yaml

    # add.cfg
    data:
      (-)key: value
      other-data: value

In the example above this will result to the following YAML:

.. code-block:: yaml

    data:
      other-data: value

.. note::

    This method of "fine grain replacements" is only useful if an original data block is big enough
    and one does not want to rewrite all of it. But in most cases it is easier to simply redefine
    the entire ``data`` one more time, as the final result, to achieve exactly the same outcome.


Updating/Replacing
^^^^^^^^^^^^^^^^^^

If there is a need to :bi:`replace` an existing element without merging with it,
it first needs to be removed, using ``(-)`` prefix. Simply remove the element
and then define a new one. Example:

.. code-block:: yaml
  :caption: Replacing a value

  # Completely remove the whole block
  (-)some_block:

  # Define a new one
  some_block:
    my_new: data
