Built-in Modules
================

.. note::

    This document lists all available built-in modules.

Overview
--------

Purpose
^^^^^^^

A module in Sysinspect is a standalone executable that receives JSON on stdin
and returns JSON on stdout. Modules can inspect, enforce, or query system state
— from installing packages to managing firewall rules.

Module Types
^^^^^^^^^^^^

- **Control modules** are Rust (or Lua) binaries that perform a specific system
  operation.
- **Runtime modules** execute user-written scripts (Lua, Python, Wasm) in a
  sandboxed environment inside the Sysinspect ecosystem.

.. important::

    Refer to the communication protocol in chapter :ref:`commproto`.

System (``sys.*``)
------------------

Core system operations: processes, packages, services, users, commands,
and network interfaces.

+------------------+----------------------------------------------------------+
| Module           | Purpose                                                  |
+==================+==========================================================+
| :doc:`sys_run`   | Run arbitrary local commands with env/STDIN control      |
+------------------+----------------------------------------------------------+
| :doc:`sys_ssrun` | Run commands on remote hosts over SSH                    |
+------------------+----------------------------------------------------------+
| :doc:`sys_proc`  | Inspect and manage processes (search, start, stop)       |
+------------------+----------------------------------------------------------+
| :doc:`sys_pkg`   | Cross-platform package management (install/remove/query) |
+------------------+----------------------------------------------------------+
| :doc:`sys_service` | Cross-platform service management (start/stop/enable)  |
+------------------+----------------------------------------------------------+
| :doc:`sys_user`  | Manage users and groups (create/modify/delete)           |
+------------------+----------------------------------------------------------+
| :doc:`sys_net`   | Query network interfaces, routes, and addresses          |
+------------------+----------------------------------------------------------+

.. toctree::
   :maxdepth: 1
   :hidden:

   sys_run
   sys_ssrun
   sys_proc
   sys_pkg
   sys_service
   sys_user
   sys_net

Filesystem (``fs.*``)
---------------------

State enforcement for files, directories, content, and symlinks.

+-----------------+-----------------------------------------------------------+
| Module          | Purpose                                                   |
+=================+===========================================================+
| :doc:`fs_file`  | Create, copy, delete, inspect files. Content line ops.    |
+-----------------+-----------------------------------------------------------+
| :doc:`fs_dir`   | Ensure directory state with mode, uid, gid                |
+-----------------+-----------------------------------------------------------+

.. toctree::
   :maxdepth: 1
   :hidden:

   fs_file
   fs_dir

Configuration (``cfg.*``)
--------------------------

Artifact push/pull between master and minions.

+---------------------+------------------------------------------------------+
| Module              | Purpose                                              |
+=====================+======================================================+
| :doc:`cfg_resource` | Push/pull configuration artifacts to/from the master |
+---------------------+------------------------------------------------------+

.. toctree::
   :maxdepth: 1
   :hidden:

   cfg_resource

Network (``net.*``)
-------------------

Outbound network operations and firewall management.

+------------------+--------------------------------------------------------------+
| Module           | Purpose                                                      |
+==================+==============================================================+
| :doc:`net_http`  | One-shot HTTP client with auth, TLS, and JSON parsing        |
+------------------+--------------------------------------------------------------+
| :doc:`net_ipfw`  | Cross-platform firewall rules (pf, ipfw, nftables, iptables) |
+------------------+--------------------------------------------------------------+

.. toctree::
   :maxdepth: 1
   :hidden:

   net_http
   net_ipfw

Operating System (``os.*``)
----------------------------

Kernel-level and system-facts introspection.

+-------------------+---------------------------------------------------+
| Module            | Purpose                                           |
+===================+===================================================+
| :doc:`os_kernel`  | Kernel module management (load/unload/list)       |
+-------------------+---------------------------------------------------+
| :doc:`os_facts`   | Collect system hardware and software facts        |
+-------------------+---------------------------------------------------+

.. toctree::
   :maxdepth: 1
   :hidden:

   os_kernel
   os_facts

Runtime
-------

Sandboxed script execution in Lua, Python, and Wasm. Each runtime provides
isolation, portability, and language-specific libraries.

+---------------------+-----------------------------------------------------+
| Module              | Purpose                                             |
+=====================+=====================================================+
| :doc:`runtime_lua`  | Lua 5.4 runtime with full stdlib and site-packages  |
+---------------------+-----------------------------------------------------+
| :doc:`runtime_py3`  | Python 3.14 runtime with site-packages support      |
+---------------------+-----------------------------------------------------+

.. toctree::
   :maxdepth: 1
   :hidden:

   runtime_lua
   runtime_py3
