Event Handlers
==============

.. note::

    This document section lists and explains available event handlers.

Overview
--------

The system uses event handlers to respond to various system events. Event handlers can perform various tasks such as
sending email notifications On a specific event trigger (e.g., failure in a service or a threshold breach), the system
sends a notification email to a configured list of recipients or call an external program with predefined arguments etc.

Each event handler can be defined with the event type and the corresponding action to ensure proper handling of the
event based on system requirements.

Event Filter Format
-------------------

.. important::

    🚨 If you define the filter format incorrectly, you won't get any reaction and the event will be ignored.
    Additionally, **no logging will be emitted** unless in debug mode.

Event has the following format:

.. code-block:: text

    action/entity/state/exit-code

Keeping this in mind, following the following module structure, we can define events as following:

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



Each of these values can be also wildcarded (e.g., ``my-action/$/some-state/0``). In this case event filter will
call a specific handler only if an action "my-action" with "some-state" will be called and the result exit code matches "0".
This means, to catch all events from all actions, use ``$/$/$/$`` event filter.


Available Handlers
------------------

Here are the available event handlers:

.. toctree::
   :maxdepth: 1

   console_logger
   outcome_logger
   pipescript
