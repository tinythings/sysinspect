Command Line Interface
======================

.. note::

    This section explains the CLI usage of all components.

Overview
--------

Sysinspect consists of three main executables:

1. ``sysinspect`` — a command to send remote commands to the cluster or run models locally.
2. ``sysmaster`` — is a controller server for all the minion clients
3. ``sysminion`` — a minion client, running as ``root`` on the target

Starting a Master
-----------------

To start a Master in foreground, issue ``--start`` option:

.. code-block:: text

    sysmaster --start

This option is also good for ``systemd`` as it runs in foreground, allowing the ``systemd``
or any similar init system taking over the service management.

However, in various use cases a standalone daemon option might be a preferred way to start
the Master. To start a Master as a standalone daemon, use ``--daemon`` option:

.. code-block:: text

    sysmaster --daemon

In this case, the ``sysmaster`` will be running as a daemon, remembering its PID. Stopping
it can be done either via SIGKILL -9 or ``--stop`` option.

Registering a Minion
--------------------

If one starts a minion for the first time, an error from the Master will be returned:

.. code-block:: text

    [15/12/2024 21:43:11] - ERROR: Minion is not registered
    [15/12/2024 21:43:11] - INFO: Master fingerprint: e79bc4ea44353c0933aacf5d84bd9e31063af8280325660a439946d7a4aee4a0

To register a minion, the following process should be performed:

1. Ensure that the Master's fingerprint is indeed as in the message above.
2. Copy the fingerprint of the Master.
3. Register the minion with ``--register`` option as follows:

.. code-block:: text

    ./sysminion  --register e79bc4ea44353c0933aacf5d84bd9e31063af8280325660a439946d7a4aee4a0

Output should be something like this:

.. code-block:: text

    [15/12/2024 21:47:03] - WARN: Preferred config at  does not exist, falling back
    [15/12/2024 21:47:03] - INFO: Initialising minion
    [15/12/2024 21:47:03] - INFO: Loading system traits data
    [15/12/2024 21:47:04] - INFO: Lading network traits data
    [15/12/2024 21:47:04] - INFO: Loading trait functions
    [15/12/2024 21:47:04] - INFO: Registration request to 10.10.2.75:4200
    [15/12/2024 21:47:04] - INFO: Minion registration has been accepted

Now the minion is ready to start.

Starting a Minion
-----------------

Operation of a Minion is identical to Master. To start a minion in foreground, simply use
``--start`` option:

.. code-block:: text

    sysminion  --start

If connection was established successfully, then the last message should be "Ehlo", something like this:

.. code-block:: text

    [15/12/2024 21:48:47] - INFO: Ehlo on 10.10.2.75:4200

To start/stop a Minion in daemon mode, use ``--daemon`` and ``--stop`` respectively.

Minion can be also stopped remotely. However, to start it back, one needs to take care of the
process themselves (either via ``systemd``, manually via SSH or any other means). To stop a minion
remotely, use its System Id:

.. code-block:: text

    sysinspect --stop 30006546535e428aba0a0caa6712e225

In this case a minion with the System Id above will be stopped, while the rest of the cluster will
continue working.

Removing a Minion
-----------------

To remove a Minion (unregister) use the following command, similar to stopping it by its System Id:

.. code-block:: text

    sysinspect --unregister 30006546535e428aba0a0caa6712e225

In this case the Minion will be unregistered, its RSA public key will be removed, connection terminated
and the Master will be forgotten. In order to start this minion again, please refer to the Minion
registration.