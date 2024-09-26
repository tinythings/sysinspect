Fact Functions
==============

.. note::

    This document explains how fact functions work.

Fact functions are just traversal mechanism to fetch the value of an entity by another consumer.

Fact functions are main workhorse for actions, as they are dynamically configuring a bind
between an action and a data. These functions are not meant to provide an actual logic how to
actually process the given data model. They are only to properly link the navigation of it.

.. important::

    A fact function is **not** a logic for data processing. It is merely a helper to **navigate** the data.

List of currently supported functions:

``fact(value | @)``

  Function ``fact`` can accept the following parameters:

    - A type of ``String``, which is an ID of a specific fact. This would be a static call.
    - An ``@`` symbol, which refers to a current fact. This would be a dynamic call.

  The function will return the entire structure of a fact data as it is represented in the Model.
  It is sometimes useful if a module already "understands" the structure "as is" without adjusting it
  within an **action** consumer.


``claim(value)``

  Function ``claim`` accepts a type ``String`` with the ID of that claim.

  The function returns a defined value of that claim.

``static()``

  Function ``static`` accepts a type ``String`` with the whole absolute path with the ID of the claim.

  A path has ``.`` dot-notation, e.g. ``foo.bar.baz`` where ``baz`` is the final ID.
