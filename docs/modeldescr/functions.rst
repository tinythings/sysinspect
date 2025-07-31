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
  A path uses ``.`` dot-notation, e.g., ``foo.bar.baz`` where ``baz`` is the final ID.

``context(value)``

  The function ``context`` accepts a type ``String`` with the ID of the context variable.
  The function returns a defined value of that context. An example usage is ``context(hostname)``,
  if that context variable was defined in the context system (either passed through CLI or set in the Master setup).

Data Definition
---------------

Here you can find the details about how to use ``claim`` and ``static`` functions.

Claim Functions
^^^^^^^^^^^^^^^

Data can be defined in the ``entities`` section under specific action IDs and states. A state is just a label or
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

In this case, the ``claim(...)`` function must be called within the same context state, otherwise the value will not be found.
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
thing. Data in the default state will not automatically appear in a specific state, and vice versa. So, if you
try to get a value from the default state while you are in a specific state, it won't work.

.. note::

  The ``$`` state is not a "default value" that transparently fills in for missing states. It is a default **state**.
  If you are in a specific state (like ``my_state``) and that state is not defined in your entities, the system will
  **not** fall back to the ``$`` state automatically. You must explicitly define the state you want to use; otherwise,
  the value will not be found.

In short: always make sure your action and your data use the same state label if you want to access the right values.

Fallback values are defined with the ``?`` (question mark) symbol and they do not belong to any specific state.
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

In this case, ``fact.key`` will return ``value`` because it is defined in the fallback state ``?``, while
``otherfact.key`` will return ``othervalue`` because it is defined in the specific state ``my_state``.

Static Functions
^^^^^^^^^^^^^^^^

Static functions are a way to grab data that never changes, no matter where you are in your system. Think of
them like a street address: if you know the address, you can always find the house, no matter where you’re
standing. These functions ignore what action or state you’re in—they just go straight to the data you asked
for.

.. hint::

  If ``claim`` is like asking for something in your own room (it depends on where you are), ``static`` is
  like looking up a friend’s address in your phone and going straight there. It doesn’t matter where you
  start from—you’ll always end up at the same place.

Static functions don’t have backup or fallback values. If you ask for something that doesn’t exist at the
address you gave, you just get nothing. You can only set up static data in the ``entities`` section. Here’s
how you define a static value:

.. code-block:: yaml

  entities:
    my_entity:
      claims:
        my_state:
          - label:
              name: Fred

It doesn’t matter what you call the state here (even if it’s made up), because static functions don’t care
about states—they just want the full path to the data. The important thing is that your data is under the
``claims`` section of your entity. That’s where static functions look.

To use a static function, you give it the full path to the value you want, like this:

.. code-block:: yaml

  actions:
    my_action:
      bind:
        - my_entity
      module: mymodule
      args:
        name: static(entities.my_entity.claims.my_state.label.name)

Here, the static function will always find "Fred" and put it into the ``name`` argument, no matter what
action or state you’re in. It’s like having a shortcut that always works, as long as you spell the path
correctly!

.. important::

  Static functions require the path to start with ``entities. ...``. The path has the following structure (see
  the YAML example above):

  .. code-block:: text

    entities.<entity>.<region>.<state>.<label>.<key>
    ^        ^        ^        ^       ^       ^
    entities.my_entity.claims.my_state.label.name

Conditional Function Processing
-------------------------------

The model supports Jinja2 templating syntax and conditions.

.. important::

  This is not real Jinja2 templating, only a simplified version without a real Python interpreter behind it!

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

Of course, under these circumstances, ``context(somevalue)`` and ``{{ context.somevalue }}`` are the same thing,
just one is used as a fact function, and the other is direct access to the context variable using Jinja syntax.

