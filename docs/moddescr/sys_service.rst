``sys.service``
================

.. note::

    This document describes ``sys.service`` module usage.

Synopsis
--------

The ``sys.service`` module manages operating-system services: start them, stop
them, restart them, reload their configuration, enable them at boot, disable
them, or simply check their current state. It works across FreeBSD, OpenBSD,
NetBSD, Linux, macOS and Android by detecting whichever init system is running
and speaking its language.

The module is a thin engine: all service-manager knowledge lives in YAML,
shipped inside the binary at compile time. If you need to teach it a new init
system or override a built-in command template, you drop a single
``sys.service.yaml`` file as a Sysinspect library and the module picks it up
automatically — no recompilation, no Rust, no ceremony.

Philosophy
----------

``sys.service`` provides telemetry, not decisions. Every operation —
including mutations like ``start`` and ``stop`` — returns a structured
data payload containing the service name, the manager that handled it,
the running state, any PIDs found, exit codes, stdout and stderr.

This data is intentionally rich so that model-level conditions,
constraints and event handlers can consume it for OpenTelemetry
forwarding, conditional chaining or compliance checks. The module itself
never decides *whether* to act; it acts when told and reports what
happened.

Usage
-----

The following options are available:

  ``check``
    Inspect a service without changing anything. Always returns retcode 0
    and a full telemetry payload (name, running, pids, manager). This is
    the operation you use for monitoring and inquiry.

  ``status``
    Check whether a service is running. Returns retcode 0 when running,
    1 when not. Includes the same telemetry fields as ``check``.

  ``start``
    Start a service.

  ``stop``
    Stop a service.

  ``restart``
    Restart a service.

  ``reload``
    Reload a service's configuration without a full restart. Not all init
    systems support this; the module reports an error when it is unavailable.

  ``enable``
    Enable a service to start at boot. Not all init systems support this.

  ``disable``
    Disable a service from starting at boot.

  ``dry-run``
    Print the command that *would* be executed without running it. Useful
    for verifying which service manager was detected and what template it
    resolved to.

The following keyword arguments are available:

  ``name`` (type: string, required)
    Service name as recognised by the init system (e.g. ``sshd``, ``nginx``,
    ``cron``).

Supported Init Systems
----------------------

+------------------+---------------------------------------------------+
| Platform         | Detected Manager(s)                               |
+==================+===================================================+
| FreeBSD          | rc.d via ``service(8)``                           |
+------------------+---------------------------------------------------+
| OpenBSD          | ``rcctl``                                         |
+------------------+---------------------------------------------------+
| NetBSD           | rc.d via ``service(8)``                           |
+------------------+---------------------------------------------------+
| Linux            | systemd, OpenRC, runit, s6, SysV init, Busybox    |
|                  | init (detected in that order)                     |
+------------------+---------------------------------------------------+
| macOS            | ``launchctl``                                     |
+------------------+---------------------------------------------------+
| Android          | ``setprop ctl.start`` / ``ctl.stop``              |
+------------------+---------------------------------------------------+

Detection on Linux runs through the list in the order shown above: it tries
``systemctl --version`` first (because systemd is overwhelmingly common), then
falls back to ``rc-service --version``, ``sv --version``, ``s6-svc -h``, and
finally ``test -x /etc/init.d/{name}``. The first manager whose detection
command exits successfully is used for all subsequent operations.

Custom Service Managers
-----------------------

Users can override any built-in manager or add a completely new one by
placing a ``sys.service.yaml`` file in the Sysinspect library directory
(``$SHARELIB/lib/sys.service.yaml``). The file uses the same schema as the
embedded YAML:

.. code-block:: yaml

    # $SHARELIB/lib/sys.service.yaml
    managers:
      my-custom-init:
        os: linux
        detect: "test -x /opt/myinit/bin/ctl"
        description: "My Special Init System"
        start: "/opt/myinit/bin/ctl start {name}"
        stop: "/opt/myinit/bin/ctl stop {name}"
        restart: "/opt/myinit/bin/ctl restart {name}"
        status: "/opt/myinit/bin/ctl status {name}"

      linux-systemd:
        os: linux
        detect: "systemctl --version"
        start: "sudo systemctl start {name}"
        stop: "sudo systemctl stop {name}"
        restart: "sudo systemctl restart {name}"
        status: "systemctl is-active {name}"
        enable: "sudo systemctl enable {name}"
        disable: "sudo systemctl disable {name}"

Entries with the same manager ID as a built-in entry replace it entirely
(the ``sudo systemctl`` example above overrides the default ``systemctl``
commands to use ``sudo``). Entries with new IDs are appended and become
available on the OS they declare.

Install the file as a library:

.. code-block:: text

    sysinspect module -A -l --path /path/to/sys.service.yaml

Examples
--------

Inspect a service and get full telemetry (always retcode 0):

.. code-block:: yaml

    actions:
      inspect-sshd:
        module: sys.service
        bind:
          - target-host
        state:
          $:
            opts:
              - check
            args:
              name:
                - sshd

Check whether a service is running (retcode 0 = running, 1 = not):

.. code-block:: yaml

    actions:
      verify-sshd:
        module: sys.service
        bind:
          - target-host
        state:
          $:
            opts:
              - status
            args:
              name:
                - sshd

Start a service:

.. code-block:: yaml

    actions:
      start-nginx:
        module: sys.service
        bind:
          - target-host
        state:
          $:
            opts:
              - start
            args:
              name:
                - nginx

Enable and start a service at boot:

.. code-block:: yaml

    actions:
      enable-cron:
        module: sys.service
        bind:
          - target-host
        state:
          $:
            opts:
              - enable
              - start
            args:
              name:
                - cron

Dry-run a restart to see what command would be used:

.. code-block:: yaml

    actions:
      preview-restart:
        module: sys.service
        bind:
          - target-host
        state:
          $:
            opts:
              - restart
              - dry-run
            args:
              name:
                - sshd

Returning Data
--------------

``check``
    Rich telemetry, always retcode 0. The ``pids`` field is a list of
    process IDs found via ``pgrep``.

    .. code-block:: json

        {
          "retcode": 0,
          "message": "Service 'sshd' is running",
          "data": {
            "name": "sshd",
            "running": true,
            "manager": "linux-systemd",
            "pids": [1234, 1235],
            "exit_code": 0
          }
        }

``status``
    Same data as ``check`` but retcode reflects the running state (0 =
    running, 1 = not running).

``start`` / ``stop`` / ``restart`` / ``reload`` / ``enable`` / ``disable``
    Mutation operations return telemetry alongside the outcome.

    .. code-block:: json

        {
          "retcode": 0,
          "message": "Service 'nginx' start successful",
          "data": {
            "name": "nginx",
            "manager": "linux-systemd",
            "exit_code": 0,
            "stdout": ""
          }
        }
