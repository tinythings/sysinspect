Secure Transport Tutorial
=========================

This tutorial walks through the normal operator lifecycle for secure transport:

1. first registration
2. fingerprint verification
3. automatic key provisioning
4. Web API TLS usage
5. recovery from a broken or replaced master/minion

First Bootstrap
---------------

Start ``sysmaster`` first.

Then start the minion once. If this is the first contact, the minion will not
be registered yet and will print the master fingerprint.

Typical pattern:

.. code-block:: text

   ERROR: Minion is not registered
   INFO: Master fingerprint: <fingerprint>

At this point:

- do not trust the fingerprint blindly
- verify it through your normal out-of-band process

Fingerprint Verification
------------------------

After you have verified the fingerprint, register the minion:

.. code-block:: bash

   sysminion --register <master-fingerprint>

If registration succeeds, the master accepts the minion RSA identity and both
sides create managed transport metadata automatically.

What gets provisioned automatically:

- the minion stores the trusted master public key
- the master stores the minion public key
- the minion creates ``transport/master/state.json``
- the master creates ``transport/minions/<minion-id>/state.json``

Normal Startup After Registration
---------------------------------

Once registration exists, start the minion normally:

.. code-block:: bash

   sysminion --start

The normal sequence is:

1. the minion loads its managed transport state
2. the minion sends secure bootstrap to the master
3. the master validates identity, version, and replay rules
4. the connection switches to a secure session
5. traits, commands, events, and sync control traffic use that secure session

Verify it from the operator side:

.. code-block:: bash

   sysinspect network --status

Look for:

- a current handshake timestamp
- an active key id
- idle rotation state unless you intentionally staged rotation

Automatic Key Provisioning
--------------------------

You do not need to create transport session keys manually.

Sysinspect manages:

- registration trust anchors
- transport metadata
- fresh per-connection secure sessions
- staged and applied rotation state

If you rotate transport state:

.. code-block:: bash

   sysinspect network --rotate --id <minion-id>

the reconnect and secure bootstrap after that rotation are still automatic.

Web API TLS Usage
-----------------

The Web API is separate from the Master/Minion secure transport.

Configure it on the master:

.. code-block:: yaml

   config:
     master:
       api.enabled: true
       api.tls.enabled: true
       api.tls.cert-file: etc/web/api.crt
       api.tls.key-file: etc/web/api.key

Then restart ``sysmaster`` and open:

.. code-block:: text

   https://<host>:4202/doc/

Normal API flow:

1. authenticate over HTTPS
2. receive a bearer token
3. send plain JSON requests over HTTPS with ``Authorization: Bearer <token>``

Broken Minion Recovery
----------------------

If a minion loses trust data or is rebuilt:

1. start it once and inspect the failure
2. if needed, unregister the old relationship on the master
3. verify the current master fingerprint again
4. register the minion again
5. start it normally
6. verify secure handshake with ``sysinspect network --status``

Broken Master Or Replaced Master Recovery
-----------------------------------------

If the master identity changes, the old trust relationship is no longer valid.

Recovery flow:

1. start the rebuilt master
2. verify its new fingerprint
3. re-register affected minions against the new master fingerprint
4. start the minions normally
5. verify transport status and, if desired, run a cluster sync

Quick Checklist
---------------

For healthy secure operation:

- verify the master fingerprint during registration
- avoid editing transport state files manually
- use ``network --status`` to confirm handshakes and rotation state
- keep Web API TLS configured separately from the Master/Minion transport
