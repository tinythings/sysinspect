Fact Functions
==============

.. note::

    This document explains how fact functions work.

Fact functions are simply traversal mechanisms used to fetch the value of an entity for another consumer.

Fact functions are the main workhorses for actions, as they dynamically configure a bind between an action
and data. These functions are not meant to provide the actual logic for processing the given data model;
they only serve to properly link its navigation.

.. important::

    A fact function is **not** a logic for data processing. It is merely a helper to **navigate** the data.

List of currently supported functions:

``fact(value)``

  Function ``fact`` can accept the following parameters:

    - A type of ``String``, which is an ID of a specific fact. This would be a static call.
    - An ``@`` symbol, which refers to a current fact. This would be a dynamic call.

The function will return the entire structure of fact data as it is represented in the Model. This can be
useful when a module already "understands" the structure "as is," without needing to adjust it within an
action consumer.


``claim(value)``

  Function ``claim`` accepts a type ``String`` with the ID of that claim.

  The function returns a defined value of that claim.

``static(value)``

  Function ``static`` accepts a type ``String`` with the whole absolute path with the ID of the claim.

  A path has ``.`` dot-notation, e.g. ``foo.bar.baz`` where ``baz`` is the final ID.
