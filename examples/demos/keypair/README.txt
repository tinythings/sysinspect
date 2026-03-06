To setup this demo, do the following:


Master
======

1. Copy model.cfg to the master's models root into its subdirectory "keypair",
   so you end up with "$MASTER/data/models/keypair/model.cfg

2. Copy sensors.cfg to the master's sensor root into its subdirectory "keypair",
   so you end up with "$MASTER/data/sensors/keypair/sensors.cfg

3. Edit config, adding this model and sensors (among other configuration):

   config:
     master:
       fileserver.models:
         - keypair
       fileserver.sensors:
         - keypair

4. Ensure you have sys.run and cfg.resource modules in the master repository:

      sysinslect module -L

   You should have "cfg.resource" and "sys.run" modules available.

Master setup is done.


Minion
======

Nothing, just start it. It has to just autosync.



To demo, bootstrap the cluster first:

   sysinspect keypair/keypair-files/bootstrap '*'


Now, start a watchdog (for fun) in another terminal:

   watch -n 1 'ls -alh /tmp/keypairdemo'


And now go and delete some stuff in /tmp/keypairdemo,
watch what happens on minion logs and watchdog.
