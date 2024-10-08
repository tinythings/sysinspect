Modules Development
===================

.. note::

    This document describes how to develop own Modules

.. toctree::
   :maxdepth: 1

   proto
   modstruct

Overview
--------

When choosing architecture for Modules, it was important to address the following
concerns:

- Unpredictable environments can require different runtime constraints
- It must be as simple as possible to extend SysInspect with own custom modules, enabling unequal programming skill levels

Therefore, Modules for SysInspect are basically a standalone programs on their own, communicating
via protocol in JSON format. Data exchange channel is done via STDIN/STDOUT. One can develop them
in any language or scripts, as long as a Module is supporting defined communication protocol.
This approach enables everyone to be as flexible and free as possible, adapting to any unpredictable
environment and allowing to choose any technology one might like to.

Runtime Modes
=============

There are several ways of running Modules on the system:

- Local
- Over SSH (or in some cases over telnet, remote shell, serial etc)
- Remote passive local, via agent

Each of these modes has their own limitations, advantages, needs and purposes.
