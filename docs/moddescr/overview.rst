Built-in Modules
================

.. note::

    This document lists all available built-in modules.

Overview
--------

Modules in Sysinspect are simply standalone executables. They can be scripts,
programs etc. However there are few rules that needs to be complied:

- All communication between Sysinspect and a module is done via JSON (STDIN/STDOUT).
- An executable must always accept data from STDIN on default.
- An executable must return help documentation when ``-h`` or ``--help`` option is passed.

.. important::

    Refer to the detailed communication protocol documentation in chapter :ref:`commproto`.


Available modules
-----------------

Below is a list of available modules and their documentation:

.. toctree::
   :maxdepth: 1

   sys_proc
   sys_net
