.. raw:: html

   <style type="text/css">
     span.underlined {
       text-decoration: underline;
     }
     span.bolditalic {
       font-weight: bold;
       font-style: italic;
     }
   </style>

.. role:: u
   :class: underlined

.. role:: bi
   :class: bolditalic

.. _configuration:

Configuration
=============

.. note::
    This document describes the configuration of Sysinspect system.

Sysinspect can run in two modes:

- **Distributed Mode** using network-connected Minions, allowing many "boxes" to be subscribed
  to a Master command center. This is a typical use.
- **Solo Mode**, i.e. locally, only on the current local "box", affecting nothing else. This usage
  is for a very small embedded systems only.

Config Location
---------------

Configuration files can be in three locations and are searched in the following order:

1. Current directory from which the app was launched: ``sysinspect.conf``.
2. "Dot-file" in the current user's home ``~/.sysinspect``
3. As ``/etc/sysinspect/sysinspect.conf``.

Synopsis
--------

Configuration is located under ``config`` section in a YAML file. This section
has two important sub-sections:

- ``master`` for all settings of Sysinspect Master
- ``minion`` for covering settings of Sysinspect Minion

Config Section
--------------

Main section of the entire configuration is ``config``. It is located at the root
of the configuration file and contains the following directives:

``modules``

    Path to location of the modules, used in the model and states. Default
    value is ``/usr/share/sysinspect/modules`` according to the LSB standard.

``master``

    Sysinspect Master configuration.

``minion``

    Sysinspect Minion configuration.


Master
^^^^^^

Sysinspect Master configuration is located under earlier mentioned ``master`` section,
and contains the following directives:

``socket``
##########

    Path for a FIFO socket to communicate with the ``sysinspect`` command,
    which is issuing commands over the network.

    Default value is ``/tmp/sysinspect-master.socket``.

``bind.ip``
###########

    IPv4 address on which the Master is listening for all incoming and outgoing traffic
    with Minion communication.

    Default value is ``0.0.0.0``.

``bind.port``
#############

    Network port number on which the Master is listening using ``bind.ip`` directive.

    Sysinspect Master port is ``4200``.


.. important::

    Master runs a **File Server service**. This service is :bi:`very important` for all the minions,
    as they are exchanging data with the master, by downloading all the required artefacts to be
    processed on their targets.

File Server service serves static data, which is continuously checked by each minion and updated,
if that data changes. In particular, the artefacts are modules, trait configs, models, states etc.
Typically, File Server service has the root of all the data in ``/etc/sysinspect/data``.

.. warning::
    Even though as of current version, there is no specific layout of the static data on the
    File Server service to manager all the artifacts. However, this is a **subject to change**.

Within the *"/data"* directory, *currently* one is free to organise the layout as they want.
However, it is :bi:`strongly` advised to keep all the models, states and other artefacts
separated from each other, using their own directories and namespaces. Future releases will have
configurable default namespaces for each cathegory of the artefacts.

Below are directives for the configuration of the File Server service:

``fileserver.bind.ip``
######################

    Same as ``bind.ip``, but for the internal File Server service.

``fileserver.bind.port``
########################

    Network port number on which the File Server service is listening.

    File Server service port is ``4201``.

``fileserver.models.root``
##########################

    Relative path where are the master models kept.

``fileserver.models``
######################

    List of subdirectories within ``fileserver.models.root``, exporting models. If a model is not
    in the list, it will not be available for the minions.

``telemetry.location``
######################

    Location of the telemetry local database. This is a directory, where the
    key/value database is located and records all results, coming from the minion
    when processing a given query. Default is set to ``/var/tmp/sysinspect/telemetry``.

``scheduler``
#############

    Scheduler is a component of Sysinspect Master, which is responsible for
    scheduling the *repetitive* tasks to call the minions. The aggregate *"scheduler"*
    takes a list of tasks. Each task is a list of key/value pairs:

    - ``name`` — name of the task
    - ``query`` — query to be executed on the minion. Query is written in a semicolon-separated format
        sending the following information:
        - model name
        - target scope (e.g. ``*`` for all targets)
    - ``traits`` — list of traits to be used for the query. E.g. ``system.os.name:Ubuntu``.
    - ``interval`` — interval of the task, i.e. how often the task should be executed.
      This value can be in seconds, minutes or hours.
    - ``interval.unit`` — unit of the interval. This value can be one of the following:

        - seconds
        - minutes
        - hours
        - days

    An example of scheduled tasks:

    .. code-block:: yaml

        - name: "Name of your task"

          # Same query as in the command line of SysInspect
          query: "foo/bar;*"
          traits:
            - system.os.name:Ubuntu
            - system.os.version:20.04
          interval: 3
          interval.unit: seconds

        - name: "Name of your another task"
          query: "some/model/etc;*"
          interval: 1
          interval.unit: minutes


Example configuration for the Sysinspect Master:

.. code-block:: yaml

    config:
        master:
            socket: /tmp/sysinspect-master.socket
            bind.ip: 0.0.0.0
            bind.port: 4200

            fileserver.bind.ip: 0.0.0.0
            fileserver.bind.port: 4201

            fileserver.models.root: /models
            fileserver.models:
              - my_model
              - my_other_model


Minion
^^^^^^

Sysinspect Minion configuration is located under earlier mentioned ``minion`` section,
and contains the following directives:

``path.root``
#############

    Typically, Minion if running standard, the root of all data kept by a Minion is
    defaulted to ``/etc/sysinspect``, same as Master. However, in an embedded and custom
    systems this might not be possible, especially if the system is usually read-only
    and writable directories are limited to only a few. In this case *root* must be
    set according to the system setup.

``path.id``
###########

    By default, the minion Id is the ``/etc/machine-id``. However, this file is usually
    present on a regular Linux server and desktop distributions, but practically never
    on the embedded systems. For this reason, the alternative location of the ``machine-id``
    needs to be specified. On many embedded Linux systems and Android, usually ``/etc`` is
    read-only, and very few places are allowed to be written.

    This option takes one of the following:

    - An absolute path to an existing ``machine-id`` file
    - ``relative`` keyword, so it is ``$MINION_ROOT/machine-id``, which is ``/etc/sysinspect/machine-id``
      by default.

    .. code-block:: yaml

        id.path: </absolute/path>|relative

``path.sharelib``
#################

    The location of sharelib directory, which is by default is at the location
    ``/usr/share/sysinspect``. On most embedded systems those root filesystem is usually read-only,
    this location can be changed. This directory contains ``lib`` and ``modules`` subdirectories.

``master.ip``
#############

    Corresponds to ``bind.ip`` of Master node and should be identical.

``master.port``
###############

    Corresponds to ``bind.ip.port`` of Master node and should be identical. By default it is
    set to ``4200``.

``master.fileserver.port``
##########################

    Port of Master's fileserver. By default it is set to ``4201``.

``master.reconnect``
####################

    Sets reconnection to the master (or not). This is a boolean value, which is set to ``true`` by default.

``master.reconnect.freq``
#########################

    Sets the frequency of reconnection to the master. This is a number of times, which is set to ``0`` by default.
    There are two options:

        - ``0`` — infinite reconnection attempts
        - ``n`` — number of reconnection attempts. If the number is reached, the minion will stop trying to reconnect.

``master.reconnect.interval``
#############################

    Interval (seconds) between reconnection attempts. This is a number of seconds, which is set to ``5-30`` range by default.
    Possible values are *(seconds, between the reconnection attemps)*:

        - ``n`` — specific amount of seconds
        - ``n-n1`` — a range of randomly selected seconds within that range

``modules.autosync``
####################

    Modules are always automatically synchronised at Minion boot. However, it requires full recalculation
    of each module's SHA256 checksum and it might take a while, if you have a lot of modules and they are big.
    This value has the following options:

    - ``full`` — full recalculation of all modules' SHA256 checksums. This is the default value.

    - ``fast`` — read cached SHA256 checksums. If the checksum is not in the cache, it will be calculated and stored in the cache.

    - ``shallow`` — no recalculation of the modules' SHA256 checksums, only verify if the module file is present. However, it will not ensure that the module is what is actually expected. This is useful for the embedded systems with read-only root filesystem, where the modules are kept in the ``/usr/share/sysinspect/modules`` directory (default).

    By default it is set to ``full``.

Example configuration for the Sysinspect Minion:

.. code-block:: yaml

    config:
        minion:
            # Root directory where minion keeps all data.
            # Default: /etc/sysinspect — same as for master
            root: /etc/sysinspect
            master.ip: 192.168.2.31
            master.port: 4200

Layout of ``/etc/sysinspect``
-----------------------------

Ideally, both Master and Minion have the same location of configuration and data collection,
which is defaulted to ``/etc/sysinspect``. This directory has many objects stored and has
a specific structure and purpose. For more making paths more short, this directory will be
referred as ``$SR`` *(Sysinspect Root)*.

Common
^^^^^^

There are directories that are same on both Master and Minion:

``$SR/functions``
#################

    Directory, containing custom trait functions. They are meant to be defined on the Master side
    and then sync'ed to all the minions.

Only on Master
^^^^^^^^^^^^^^

Public and private RSA keys of Master are:

``$SR/master.rsa``
##################

    Master's private RSA key.

``$SR/master.rsa.pub``
######################

    Master's public RSA key.

``$SR/minion-keys``
###################

    Public keys from registered minions in format ``<minion-id>.rsa.pub``.

    Each registered minion has its own Id. Typically it is ``/etc/machine-id`` or automatically
    generated one, if this file does not exist.

``$SR/minion-registry``
#######################

    A binary cache of minion's data, such as minion traits, data about currently connected minions etc.
    This is fully purge-able directory, i.e. data can be freely deleted. However, Sysinspect Master
    needs to be restarted and all minions needs to reconnect.

Only on Minion
^^^^^^^^^^^^^^

Public and private RSA keys of Master are:

``$SR/master.rsa``
##################

    Minion's private RSA key.

``$SR/master.rsa.pub``
######################

    Minion's public RSA key.

``$SR/traits``
##############

    Directory, containing custom static traits of a Minion.

``$SR/models``
##############

    Directory, containing models.
