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

Inheriting Data
---------------

Data within the Model Description can be inherited in three ways:

- By extending an existing element or adding a new one
- Overriding an element
- Removing an element or its data

Adding
^^^^^^

Adding an element is as simple as that: add another element with its unique ID.
If such ID already exists, see the "Overriding" section.

For example, there are two files: ``base.cfg`` and ``add.cfg``:

.. code-block:: yaml

    # base.cfg
    data:
      key: value

.. code-block:: yaml

    # add.cfg
    other:
      key: value

They both will result into one data block:

.. code-block:: yaml

    data:
      key: value
    other:
      key: value

Extending
^^^^^^^^^

Extending an element requires explaining the engine which part of an element is extended.
To do so, an element should have ``(+)`` prefix. For example, in two files ``base.cfg`` and ``add.cfg``
an element ``data`` can be extended this way:

.. code-block:: yaml

    # base.cfg
    data:
      key: value

.. code-block:: yaml

    # add.cfg
    (+)data:
      other-key: value

.. important::

    Only datablocks can be extended. A last clashing value can be either overwritten or removed.
    The example above is read as such:

        *"Extend an existing element 'data' with the new content, keeping the original"*

That said, ``data: key:value`` will replace ``data:: key:othervalue``.


In a nutshell, the prefix ``(+)`` opens an element for "editing" and will not bluntly overwrite it.

Replacing
^^^^^^^^^

Replacing is straightforward and does not require any special syntax.
If a key with a structure already exists, it will be just replaced with a new one.

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
    (+)data:
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