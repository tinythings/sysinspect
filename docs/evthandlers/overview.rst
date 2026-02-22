Sensors and Events Routing
==========================

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


Routing Events to Handlers
---------------------------

Actions and sensors produce different events with different payloads and are handled slightly differently.
You can route those events to handlers by defining event filters.

.. toctree::
   :maxdepth: 1

   action_event_routing
   sensor_event_routing


Available Handlers
------------------

Here are the available event handlers:

.. toctree::
  :maxdepth: 1

  console_logger
  outcome_logger
  pipescript
  pipeline
  chainstop
