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

Data Definition
---------------

Data can be defined in ``entities`` section under specific action IDs and states. A state is just a label or
unspecified ``$`` *(default state)*. For example:

.. code-block:: yaml

  entities:
    my_entity:
      claims:
        my_state:
          - fact:
              key: value
          - otherfact:
              key: othervalue

In this case, ``claim(...)`` function must be called within the same context state, otherwise value will not be found.
For example:

.. code-block:: yaml

  actions:
    my_action:
      bind:
        - my_entity
      state: my_state # Important!
      module: mymodule
      args:
        foo: claim(key.fact)
        bar: claim(key.otherfact)

In this example, when you use ``claim(key.fact)`` and ``claim(key.otherfact)``, you get the values ``value`` and
``othervalue``. This works because the action is "bound" to the entity in the same state where those claims are set.

Think of a "state" as a label for a certain situation or version of your data. If you use the label ``my_state``, you
must also use ``my_state`` everywhere you want to access those values. There is also a default state, written as ``$``.
But be careful: if you are working in a specific state (like ``my_state``), the default state ``$`` is not the same
thing. Data in the default state will not automatically appear in a specific state, and the other way around. So, if you
try to get a value from the default state while you are in a specific state, it won't work.

.. note::

  The ``$`` state is not a "default value" that transparently fills in for missing states. It is a default **state**.
  If you are in a specific state (like ``my_state``) and that state is not defined in your entities, the system will
  **not** fall back to the ``$`` state automatically. You must explicitly define the state you want to use; otherwise,
  the value will not be found.

In short: always make sure your action and your data use the same state label if you want to access the right values.

Fall back values are defined with ``?`` (question mark) symbol and they are not belonging to any specific state.
They can be used to provide default values when the main value is not available. For example:

.. code-block:: yaml

  entities:
    my_entity:
      claims:
        ?:  # <-- Fallback state
          - fact:
              key: value

        my_state:
          - otherfact:
              key: othervalue

  actions:
    my_action:
      bind:
        - my_entity
      state: my_state
      module: mymodule
      args:
        foo: claim(fact.key)
        bar: claim(otherfact.key)

In this case ``fact.key`` will return ``value`` because it is defined in the fallback state ``?``, while
``otherfact.key`` will return ``othervalue`` because it is defined in the specific state ``my_state``.

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
