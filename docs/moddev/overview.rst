Modules Development
===================

.. note::

    This document describes how to develop own Modules

.. toctree::
   :maxdepth: 1

   proto
   modstruct
   pymod

Overview
--------

When choosing architecture for Modules, it was important to address the following
concerns:

- Unpredictable environments can require different runtime constraints
- It must be as simple as possible to extend SysInspect with own custom modules, enabling unequal programming skill levels

There are two kind of purposes to call the modules:

- System integration assertion
- Applying a system state

One can use either of these purposes or mix them together.

Native Modules
--------------

Modules for SysInspect are essentially a standalone programs on their own, communicating
via protocol in JSON format. Data exchange channel is done via STDIN/STDOUT. One can develop them
in any language or scripts, as long as a Module is supporting defined communication protocol.
This approach enables everyone to be as flexible and free as possible, adapting to any unpredictable
environment and allowing to choose any technology one might like to.

Python Modules
--------------

.. important::

    What!? Python? *Without* any extra kind of runtime??..

Yes!

Since version 0.2.0, SysInspect brings its own embedded Python runtime, specification 3.12. However,
this runtime comes with the limitations. It contains a "frozen" standard library and does not support
native modules. This means:

- Anything which is written in Python supposed to work
- Anything which is native (C or C++) will not work and will never be supported

On the other hand, the entire Minion with the whole Python runtime and its standard
"included batteries" costs just 29 Mb on the disk and is shipped just as one single static
binary (``musl``).
