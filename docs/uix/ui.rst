.. raw:: html

   <style type="text/css">
     span.underlined {
       text-decoration: underline;
     }
     span.bolditalic {
       font-weight: bold;
       font-style: italic;
     }
   </style>

.. role:: u
   :class: underlined

.. role:: bi
   :class: bolditalic

Termainal User Interface
========================

.. note::
   This documentation shows how to use the Terminal User Interface (TUI) in Sysinspect.

Overview
--------

Sysinspect is using a Terminal User Interface (TUI) to provide a user-friendly way to interact with the system.
While web-based Interface would be more user-friendly, TUI is a good choice for many reasons: it is easy to use,
is fast, lightweight, easy to implement, and easy to maintain. It is also a good choice for systems that do not
have a graphical environment, cannot expose HTTP/S port, or for users who prefer to work in a terminal.

Usage
-----

To start the TUI, run the following command:

.. code-block:: bash

   sysinspect -u | --ui

This command will start the TUI and display the main screen with the list of available modules.
You will see a three panel layout: the left panel shows the list of model calls, the middle panel shows the
list of minions that were affected by the model call, and the right panel shows the list of events that were
happening on the particular minion. The right panel will be empty until you select a minion from the middle panel.

Navigating the events in the right panel is done with the up/down arrow keys and ``ENTER`` to jump to the details
panel, where one can scroll the available data.

To exit the TUI, press ``q`` or ``ESC``.
