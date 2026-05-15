``net.ipfw``
============

.. note::

    This document describes ``net.ipfw`` module usage.

Synopsis
--------

The ``net.ipfw`` module manages firewall rules through a common expression
language. Write one rule once, and it translates to anything the host
runs — pf on BSD/macOS, ipfw on FreeBSD, nftables on modern Linux,
iptables on legacy Linux.

Rules are expressed as JSON objects. The module auto-detects the active
firewall backend or accepts an explicit ``--backend`` override. A
``native`` escape hatch allows raw backend-specific rules when the common
language is not expressive enough.

Usage
-----

The following options are available:

  ``check``
    List current rules for the detected backend.

  ``present``
    Ensure a rule exists. Adds the rule if it is not already present.
    Idempotent: rules are matched by content, not by position.

  ``absent``
    Remove a rule matching the expression. Searches the current ruleset
    and removes the first match. Idempotent: returns success if no
    match exists.

  ``flush``
    Remove all rules for the detected backend. Destructive, irreversible.

  ``dry-run``
    Print the translated rule without executing it. Works without root
    privileges and without a live firewall backend.

The following keyword arguments are available:

  ``action`` (type: string)
    ``allow`` or ``deny``. Required for ``present`` and ``absent``.

  ``protocol`` (type: string)
    ``tcp``, ``udp``, ``icmp``, or ``any``. Default: ``tcp``.

  ``port`` (type: string)
    Single port number, e.g. ``80``.

  ``port-range`` (type: string)
    Port range, e.g. ``8000-8080``. Mutually exclusive with ``port``.

  ``source`` (type: string)
    Source IP or CIDR. Default: ``any``.

  ``destination`` (type: string)
    Destination IP or CIDR. Default: ``any``.

  ``interface`` (type: string)
    Network interface, e.g. ``eth0``, ``em0``.

  ``direction`` (type: string)
    ``in`` or ``out``. Default: ``in``.

  ``stateful`` (type: bool)
    Enable connection state tracking. Default: ``false``.

  ``log`` (type: bool)
    Log matching packets. Default: ``false``.

  ``backend`` (type: string)
    Force a specific backend. One of ``pf``, ``ipfw``, ``nftables``, ``iptables``.
    Auto-detected if omitted.

  ``comment`` (type: string)
    Human-readable rule description. Informational only.

  ``native`` (type: object)
    Raw backend rules keyed by backend name. When present, takes
    precedence over the common expression fields.

Examples
--------

Allow HTTP on port 80 (any OS):

.. code-block:: yaml

    actions:
      allow-http:
        module: net.ipfw
        bind:
          - web-servers
        state:
          $:
            opts:
              - present
            args:
              action: allow
              port: "80"
              stateful: true

Block outbound SMTP:

.. code-block:: yaml

    actions:
      block-smtp-out:
        module: net.ipfw
        bind:
          - all-servers
        state:
          $:
            opts:
              - present
            args:
              action: deny
              protocol: tcp
              port: "25"
              direction: out

Allow SSH from a management subnet with logging:

.. code-block:: yaml

    actions:
      allow-ssh-mgmt:
        module: net.ipfw
        bind:
          - all-servers
        state:
          $:
            opts:
              - present
            args:
              action: allow
              port: "22"
              source: 10.0.0.0/8
              stateful: true
              log: true
              comment: "SSH from management VLAN"

Block ICMP (ping):

.. code-block:: yaml

    actions:
      block-ping:
        module: net.ipfw
        bind:
          - all-servers
        state:
          $:
            opts:
              - present
            args:
              action: deny
              protocol: icmp

Native pf anti-spoofing rules:

.. code-block:: yaml

    actions:
      pf-antispoof:
        module: net.ipfw
        bind:
          - freebsd-servers
        state:
          $:
            opts:
              - present
            args:
              native:
                pf: |
                  scrub in all
                  antispoof quick for egress

Remove a rule:

.. code-block:: yaml

    actions:
      remove-http:
        module: net.ipfw
        bind:
          - web-servers
        state:
          $:
            opts:
              - absent
            args:
              action: allow
              port: "80"

Inspect current rules:

.. code-block:: yaml

    actions:
      audit-firewall:
        module: net.ipfw
        bind:
          - all-servers
        state:
          $:
            opts:
              - check
            args: {}

Dry-run to preview a rule without applying it:

.. code-block:: yaml

    actions:
      preview-rule:
        module: net.ipfw
        bind:
          - all-servers
        state:
          $:
            opts:
              - present
              - dry-run
            args:
              action: deny
              port-range: "8000-8080"

Flush all rules (use with extreme caution):

.. code-block:: yaml

    actions:
      reset-firewall:
        module: net.ipfw
        bind:
          - all-servers
        state:
          $:
            opts:
              - flush
            args: {}

Returning Data
--------------

``check``
  Returns the current ruleset:

  .. code-block:: json

      {
        "retcode": 0,
        "message": "pf rules listed",
        "data": {
          "rules": [
            "pass in quick proto tcp from any to any port 80",
            "block in quick proto icmp from any to any"
          ]
        }
      }

``present``
  Returns the added rule:

  .. code-block:: json

      {
        "retcode": 0,
        "message": "Firewall rule added (pf)",
        "data": {
          "rule": "pass in quick proto tcp from any to any port 80",
          "backend": "pf"
        }
      }

``absent``
  Returns confirmation of removal:

  .. code-block:: json

      {
        "retcode": 0,
        "message": "pf rule removed"
      }

  If no matching rule is found:

  .. code-block:: json

      {
        "retcode": 1,
        "message": "No matching pf rule found"
      }

``flush``
  Returns confirmation:

  .. code-block:: json

      {
        "retcode": 0,
        "message": "All pf rules flushed"
      }

``dry-run``
  Previews the translated rule without executing:

  .. code-block:: json

      {
        "retcode": 0,
        "message": "[dry-run] would add rule (pf): pass in quick proto tcp from any to any port 80"
      }

Supported Backends
------------------

+-----------+------------------------------------------------+
| Backend   | Platforms                                      |
+===========+================================================+
| pf        | FreeBSD, OpenBSD, NetBSD, macOS                |
+-----------+------------------------------------------------+
| ipfw      | FreeBSD                                        |
+-----------+------------------------------------------------+
| nftables  | Modern Linux (kernel 3.13+)                     |
+-----------+------------------------------------------------+
| iptables  | Legacy Linux                                   |
+-----------+------------------------------------------------+

The module probes backends in the order listed above. The first one
that responds successfully is selected. Override with the ``--backend``
argument.

.. note::

    ``present`` and ``absent`` require root privileges on all platforms.
    ``dry-run`` works without root and without a live firewall backend.
