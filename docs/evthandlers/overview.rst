Event Handlers
==============

.. note::

    This document section lists and explains available event handlers.

.. toctree::
   :maxdepth: 1

   console_logger
   outcome_logger

Overview
--------

The system uses event handlers to respond to various system events. Event handlers can perform various tasks such as
sending email notifications On a specific event trigger (e.g., failure in a service or a threshold breach), the system
sends a notification email to a configured list of recipients or call an external program with predefined arguments etc.

Each event handler can be defined with the event type and the corresponding action to ensure proper handling of the
event based on system requirements.
