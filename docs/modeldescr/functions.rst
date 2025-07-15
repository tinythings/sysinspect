Fact Functions
==============

.. note::

  This document explains how fact functions work.

Fact functions are simply traversal mechanisms used to fetch the value of an entity for another consumer.

Fact functions are the main workhorses for actions, as they dynamically configure a bind between an action
and data. These functions are not meant to provide the actual logic for processing the given data model;
they only serve to properly link its navigation.

.. important::

  A fact function is **not** logic for data processing. It is merely a helper to **navigate** the data.

List of currently supported functions:


``claim(value)``

  The function ``claim`` accepts a type ``String`` with the ID of that claim.
  The function returns a defined value of that claim.

``static(value)``

  The function ``static`` accepts a type ``String`` with the whole absolute path with the ID of the claim.
  A path has ``.`` dot-notation, e.g., ``foo.bar.baz`` where ``baz`` is the final ID.

``context(value)``

  The function ``context`` accepts a type ``String`` with the ID of the context variable.
  The function returns a defined value of that context. An example usage is ``context(hostname)``,
  if that context variable was defined in the context system (either passed through CLI or set in the Master setup).

Conditional Function Processing
-------------------------------

If there is a function, say ``context(foo)``, that returns a value, but you want to check if that value is defined,
you can use the template conditional syntax:

.. code-block:: jinja

  {% if context.father is defined and context.father == "Darth Vader" %}
    {{ context.father }}
  {% endif %}

For example, this technique can be used to define module parameters based on the context variables:

.. code-block:: jinja

  run-something:
    descr: My great module
    module: mystuff.something
    bind:
      - my_entity
    state:
      $:
      opts:
        - doit
      args:
      {% if context.tgt is defined and context.metaid is defined %}
        bar: "context(metaid)"
        baz: "context(tgt)"
      {% endif %}

Of course, under these circumstances, the ``context(somevalue)`` and ``{{ context.somevalue }}`` are the same thing,
just one is used as a fact function, and the other is direct access to the context variable using Jinja syntax.
