Python Modules
==============

.. note::

    This document describes how to develop modules in Python.

Overview
========

Native modules are fine, but not everyone will like to craft them and distribute them
on the constrained embedded environments. For this reasons, SysInspect brings the whole
entire Python 3.12 embedded runtime.

Installation
============

None. As long as SysInspect Minion is running, it is ready to execute any pure Python
code.

Python Module Tutorial
======================

This is a very short self-explanatory Tutorial how to make a Python module for SysInspect.

Principle
---------

Writing Python module is essentially boils down to the same principle as native modules:

1. Do whatever you need to do
2. Return a proper JSON structure back

Hello, world!
-------------

To write a very simple "Hello, world!" module, you will need to use ``main(*args, **kw)``
function at the module level, which will be called to execute everything. The ``args`` and ``kw``
is what comes from the Action definition. The rest is totally relying on the module logic.

A very minimal module would look like so:

.. code-block:: python

    # 1. Get a return Object from the standard library
    from sysinspect import SysinspectReturn

    # 2. Define main function
    def main(*args, **kw) -> str:
        """
        Main function to dispatch the module.
        """

        # 3. Instantiate return object
        r = SysinspectReturn()

        # 4. Add data that needs to be returned back
        r.add_data({"hello", "world!"})

        # 5. Return it as a string, calling str() on that object!
        return str(r)

Module can also take advantage of SysInspect core and use current Minion traits. This way
the module can be more flexible and allow more control. For example, this module would return
the CPU brand of the Minion:

.. code-block:: python

    from sysinspect import SysinspectReturn
    from syscore import MinionTraits

    def main(*args, **kw) -> str:
        r = SysinspectReturn()
        r.add_data({
            "cpu": MinionTraits().get("hardware.cpu.brand")
            }
        )

        return str(r)

Hacking
-------

In order to call own Python module and work on it, the easiest way is to just define an
action in an own model and start calling it locally, using ``sysinspect`` utility:

.. code-block:: shell

    sysinspect -m <path/to/the/model> -e <entity> -s <state>

Ansible Integration
===================

Essentially, the module is ``modules/cfg/bridge.py`` and is called as ``cfg.mgmt``. It can
call only Ansible Modules (not Ansible Action Modules!) and is called quite straightforward:

.. code-block:: yaml

    actions:
        ansible-module:
            descr: Call an Ansible module
            module: cfg.bridge
            bind:
            - addresses

            state:
            interfaces:
                opts:
                    # Module name
                    - copy
                args:
                    # Module args, just like in Ansible playbook
                    src: /etc/networks
                    dest: /tmp/networks.copy
                    mode: 0400

In this case, an Action which is bound to an Entity ``addresses`` will start bridge module,
which will call an Ansible built-in module ``copy`` with all the required args for it.

Get Minion Configuration
========================

Sometimes the module needs to know current minion configuration. Of course, one can just
read it all the time in ``/etc/sysinspect/sysinspect.conf`` and it could be mostly correct,
unless the entire minion is not started from some other configuration.

However, the minion already has configuration parsed, and it can be just reused inside the
Python script like so:

.. code-block:: python

    from syscore import MinionConfig

    def main(*args, **kw) -> str:
        cfg = MinionConfig()
        print("Master address:", cfg.master_addr())
        print("Fileserver address:", cfg.fileserver_addr())

        # More methods
        print(dir(cfg))

        return "{}"
