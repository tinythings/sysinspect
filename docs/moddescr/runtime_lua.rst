``runtime.lua``
===============

.. note::

    This document describes ``runtime.lua`` module usage.

Synopsis
--------

This plugin is intended to provide Lua runtime environment
for user-written Lua scripts to be executed inside Sysinspect ecosystem.

Usage
-----

Options:

  list
    List of available Lua scripts, ready to be called as modules.

Keyword arguments:

  mod (type: string, required)
    The name of a Lua script to be executed. This script will be looked up
    in the predefined configured scripts directory.

  [ANY]
    Any other arguments (key/value pairs) passed will be forwarded to the Lua script
    being executed. Their types will be preserved (string, number, boolean, array, object).
