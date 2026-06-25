To setup this demo, do the following:


Master
======

1. Copy model.cfg to the master's models root into subdirectory "keyring",
   so you end up with "$MASTER/data/models/keyring/model.cfg"

2. Copy sensors.cfg to the master's sensors root into subdirectory "keyring",
   so you end up with "$MASTER/data/sensors/keyring/sensors.cfg"

3. Export both scopes in master config:

   config:
     master:
       fileserver.models:
         - keyring
       fileserver.sensors:
         - keyring

4. Ensure these modules exist on the master repository:

      sysinspect module -L

   You should have "cfg.resource" and "sys.run" available.


Minion
======

Nothing special. Let it sync and start normally.


To demo, bootstrap the cluster first:

   sysinspect keyring/keyring-files/bootstrap '*'


Expected result:

- each host creates its own local private/public pair under /tmp/keyringdemo
- each host publishes its own public key to the Master datastore
- each host syncs all cluster public keys into:

      /tmp/keyringdemo/pubring/


Useful watchdogs:

   watch -n 1 'find /tmp/keyringdemo -maxdepth 2 -type f | sort'


Try deleting:

- /tmp/keyringdemo/<host>.pub
- /tmp/keyringdemo/<host>.priv
- /tmp/keyringdemo/pubring/<other-host>.pub

and watch how the local host repairs its own key files and resyncs the shared
pubring.
