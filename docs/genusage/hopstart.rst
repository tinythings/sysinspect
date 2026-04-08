Hopstart Backend
================

Hopstart is the SSH-backed startup backend used when Sysinspect cannot rely on
the target host's own init system.

This is not a replacement for ``systemd``, ``rc.d``, or similar tools. It is a
fallback backend for awkward targets such as:

- read-only systems
- embedded devices
- appliances
- Android-like hosts
- environments where only one writable application tree exists

What Hopstart Means
-------------------

For a hopstart-managed minion, the master stores startup inventory in its CMDB:

- SSH user
- requested host
- installation root
- binary path
- config path
- startup backend kind

The minion's local marker also records:

.. code-block:: yaml

   root: /path/to/installation
   init: hopstart

This means the minion is expected to be started externally by the master over
SSH.

CLI
---

Issue hopstart for all eligible offline minions:

.. code-block:: bash

   sysinspect cluster --hopstart

Issue hopstart for selected hosts:

.. code-block:: bash

   sysinspect cluster --hopstart 'edge*'
   sysinspect cluster --hopstart --hostnames=android01,192.168.2.50
   sysinspect cluster --hopstart --id 30006546535e428aba0a0caa6712e225

Selection Rules
---------------

Hopstart only attempts minions that are:

- selected by the command
- currently offline
- stored in the master CMDB
- marked with backend ``hopstart``

It does not wait for full readiness. It only issues the daemon start attempt.

Master Logging
--------------

Hopstart is intentionally Unix-like and minimal.

The ``sysinspect`` command only confirms that hopstart was issued. The operator
is expected to watch the master log for lines like:

.. code-block:: text

   [07/04/2026 15:57:25] - INFO: Hop-start android01 at /storage/blyat/sysinspect as shell

Configuration
-------------

Master-side settings live under flat dotted keys in ``config.master``:

.. code-block:: yaml

   config:
     master:
       hopstart.batch: 10
       hopstart.network.forward: false
       hopstart.on-start: false

``batch``
~~~~~~~~~

Maximum number of concurrent SSH launch attempts.

Default: ``10``

``network.forward``
~~~~~~~~~~~~~~~~~~~

Reserved for future forwarding through another master or proxy tier.

Default: ``false``

``on-start``
~~~~~~~~~~~~

Reserved for future automatic hopstart when the master itself starts.

Default: ``false``

Operational Notes
-----------------

- SSH access and permissions must already work before hopstart can work.
- Hopstart assumes key/user management happens elsewhere.
- Hopstart uses the stored minion binary path and config path from CMDB.
- Hopstart starts the minion in daemon mode and returns immediately.
- If the target is already running, minion daemon mode stays idempotent.

Related Material
----------------

- :doc:`cli`
- :doc:`secure_transport`
- :doc:`operator_security`
