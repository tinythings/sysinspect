Event Routing
==============

.. note::

    This document section lists and explains available event handlers.

Overview
--------

The system uses *event handlers* to react to what happens during checks/actions ("events").

Think of an event handler as a small automation hook: when something matches your event filter, SysInspect runs a
handler that can do useful things like:

* print a message to the console/logs
* write structured output for later parsing
* call an external script/program with predefined arguments
* notify people (e.g., email/whatever your environment supports)

You configure handlers by defining an **event filter** (what to match) and a **handler action** (what to do).
If the filter matches, the handler runs. If it doesn't match, nothing happens.

Event Filter Format
-------------------

.. important::

    🚨 If you define the filter format incorrectly, you won't get any reaction and the event will be ignored.
    Additionally, **no logging will be emitted** unless in debug mode.

An event is represented as a single path-like string:

.. code-block:: text

    action/entity/state/exit-code

Where:

* ``action`` is the action ID (e.g., a check you run)
* ``entity`` is the bound entity ID (e.g., a host/service/thing you target)
* ``state`` is the state ID inside the action (e.g., ok/warn/crit or your own state name)
* ``exit-code`` is the result indicator:

  * ``0`` = success
  * ``E`` = error/failure
  * ``$`` = any exit code (match everything)

Given the module structure below, you can write event filters like this:

.. code-block:: yaml

    actions:
      my-great-action:  # This is the action ID
        ...
        bind:
          - some-entity # This is the entity ID
        state:
          some-state:   # This is the state ID
            ...

    events:
      my-great-action/some-entity/some-state/0: # React only on successes
         ...

      my-great-action/some-entity/some-state/E: # React only on errors
         ...

      my-great-action/some-entity/some-state/$: # React on all exit codes
         ...

Each segment can be wildcarded using ``$``.

Example: ``my-action/$/some-state/0``

This will trigger only when:

* the action is ``my-action``
* the state is ``some-state``
* the result exit code is ``0`` (success)
* the entity can be anything (because of the ``$``)

More practical examples:

* ``$/$/$/E``: catch *all* errors, regardless of action/entity/state
* ``deploy/prod/$/$``: catch all deploy events for the ``prod`` entity, any state, any exit code
* ``backup/$/$/0``: catch all successful backups

If you truly want to catch everything from everywhere, use ``$/$/$/$``.


Available Handlers
------------------

Here are the available event handlers:

.. toctree::
  :maxdepth: 1

  console_logger
  outcome_logger
  pipescript
  pipeline
