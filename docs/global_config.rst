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

.. _global_configuration:

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


Common Configuration
^^^^^^^^^^^^^^^^^^^^

The following configuration is applicable to both Master and Minion in their respective configuration files.

``log.stream``
##############

    Type: **string**

    Path to the log stream file. This file is where Sysinspect Master or Minion writes down
    everything it does—like keeping a diary of its actions and events. If something
    goes wrong or you want to check what happened, you can look here. The Master or Minion tries
    to save this file in a few places, depending on what it's allowed to access:

    1. ``/var/log/`` typically a standard place for logs, but might not be accessible
    2. ``$HOME/.local/`` in user home directory
    3. ``/tmp/`` if anything else fails

    Default filename for Master is ``sysmaster.standard.log`` and for Minion is ``sysminion.standard.log``.

``log.errors``
###############

    Type: **string**

    Path to the log errors file. This file is used to collect all error logs from the
    Sysinspect Master or Minion. Depends on permissions, the following order is used:

    1. ``/var/log/`` typically a standard place for logs, but might not be accessible
    2. ``$HOME/.local/`` in user home directory
    3. ``/tmp/`` if anything else fails

    Default filename for Master is ``sysmaster.errors.log`` and for Minion is ``sysminion.errors.log``.

``pidfile``
############

    Type: **string**

    Path to the PID file. This file is used to store the process ID of the Sysinspect Master or Minion.
    It is important for managing the lifecycle of the service, allowing for proper start/stop
    operations.

    By default, pidfile is located at standard location: ``/run/user/<UID>/sysmaster.pid`` for Master
    and ``/run/user/<UID>/sysminion.pid`` for Minion.

    .. note::

        Relocate the PID file to a different location only if it is really necessary: e.g.
        your system is not standard, different permissions etc. Otherwise it is highly recommended
        to keep the default location as is.


Master
^^^^^^

Sysinspect Master configuration is located under earlier mentioned ``master`` section,
and contains the following directives:

``socket``
##########

    Type: **string**

    Path for a FIFO socket to communicate with the ``sysinspect`` command,
    which is issuing commands over the network.

    Default value is ``/var/run/sysinspect-master.socket``.

``bind.ip``
###########

    Type: **string**

    IPv4 address on which the Master is listening for all incoming and outgoing traffic
    with Minion communication.

    Default value is ``0.0.0.0``.

``bind.port``
#############

    Type: **integer**

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

    Type: **string**

    Same as ``bind.ip``, but for the internal File Server service.

``fileserver.bind.port``
########################

    Type: **integer**

    Network port number on which the File Server service is listening.

    File Server service port is ``4201``.

``fileserver.models.root``
##########################

    Type: **string**

    Relative path where are the master models kept.

``fileserver.models``
######################

    Type: **list**

    List of subdirectories within ``fileserver.models.root``, exporting models. If a model is not
    in the list, it will not be available for the minions.

``api.enabled``
###############

    Type: **boolean**

    Enable or disable the WebAPI service to control Sysinspect Master remotely.

    .. important::

        The WebAPI uses ``libsodium`` for encryption instead of standard SSL/TLS. This is because embedded
        or IoT networks may lack DNS or have changing IP addresses, making SSL/TLS certificates unreliable,
        as they are tied to specific DNS names or IPs. To connect to the WebAPI, use the Sysinspect client,
        which authenticates using an RSA keypair and symmetric keys over an internal protocol.

    The URL the Swagger UI is typically is running over unencrypted plain text HTTP protocol at ``http://<HOST>:<PORT>/api-doc/``.
    Default port is ``4202``.

    .. note::

        Swagger UI is a web-based interface for the WebAPI service, allowing users to interact with the API.
        However it runs only if development mode is enabled, because it relies on unencrypted HTTP traffic
        and API requires a proper protocol interaction that cannot be achieved with Swagger UI.

        **In development mode authentication is fully disabled and no traffic is encrypted.**

    Default is ``true``.

``api.bind.ip``
################

    Type: **string**

    IPv4 address on which the WebAPI service is listening for all incoming and outgoing traffic.

    Default value is ``0.0.0.0``.

``api.bind.port``
#################

    Type: **integer**

    Network port number on which the WebAPI service is listening.

    WebAPI service port is ``4202``.

``api.auth``
############

    Type: **string**

    Authentication method to be used for the WebAPI service. This is a string and can be one of the following:

        - ``pam``
        - ``ldap`` `(planned, not implemented yet)`

``api.devmode``
################

    Type: **boolean**

    Enable or disable development mode for the WebAPI service.

    .. danger::

        This option is exclusively only for development purposes! If it is enabled, Swagger UI will be running
        on

    Default is ``false``.

``telemetry.location``
######################

    Type: **string**

    Location of the telemetry local database *(do not mix with the OTEL or OTEL collector)*.
    This is a directory, where the key/value database is located and records all results,
    coming from the minion when processing a given query. Default is set to ``/var/tmp/sysinspect/telemetry``.

``telemetry``
#############

    Type: **key/value**

    The following keys are supported:

    ``collector.grpc``
        Type: **string**

        This is the location of the telemetry collector. It is a string in format
        ``<IP>:<PORT>``. This is the location of the telemetry collector, which is
        used to send all telemetry data to. This is a string and can be one of the following:
        URI of the telemetry collector in format ``<IP>:<PORT>``. Default value is
        ``127.0.0.1:4317`` assuming that the collector is running on the same machine.

    ``collector.compression``
        Type: **string**

        Compression algorithm to be used for the telemetry collector. This is a string
        and can be one of the following:

            - ``gzip`` (default)
            - ``zstd``
            - ``none``

        Which algorithm to choose?

            - ``gzip`` is a good choice for most of the cases. It is most backward compatible but it does
              not have a good compression ratio and is using more CPU power. On large metric, ratio is
              about 33.8 and throughput is about 131 MB/s, resulting to about 52K ns/op.
            - ``zstd`` is a much better choice for the embedded systems, where the CPU power is limited.
              It has a better compression ratio, and is also faster than ``gzip``, but is too new.
              On large metric, ratio is about 47.2 and throughput is about 476 MB/s, resulting to about 14K ns/op.
            - ``none`` no compression at all. This is a good choice for the embedded systems, where the
              CPU power is limited and the network bandwidth is not an issue.

        .. attention::

            The compression algorithm must be supported by the telemetry collector.
            Far not all collectors supports ``zstd`` compression algorithm.

    ``exporter-resources``
        Type: **key/value**

        This is a key/value pair, which is used to set the resources for the telemetry exporter. This element
        can contain any kind of static values. A resource describes the entity producing telemetry (e.g. a process,
        container, or service). It is immutable and shared by all signals (traces, metrics, logs) coming from that entity.

        The default included keys are the following:

            - ``service.name`` (string) — name of the telemetry service. Default value is ``sysinspect``.
            - ``service.namespace`` (string) — namespace of the telemetry service. Default value is ``sysinspect``.
            - ``service.version`` (string) — version of the telemetry service. Default value is the current SysInspect version.
            - ``host.name`` (string) — name of the host. Default value is the hostname of the machine.
            - ``os.type`` (string) — type of the operating system. Default value is ``linux``.
            - ``deployment.environment`` (string) — deployment environment of the operating system. Default value is ``production``.
            - ``os.version`` (string) — version of the operating system. Default value is the current OS version.

        .. attention::

            To turn off a specific resource from being exported (redefined or default), not specifying it will lead to a default
            value. In order to explicitly disable a default resource, set the value to ``false``.

    ``exporter-scope``
        Type: **key/value**

        This is a key/value pair, which is used to set the queue for the telemetry exporter. Scope are typically a name, version,
        schema_url attributes etc. The attributes here are optional, build-time metadata that further qualify the scope—e.g.
        the URL of the instrumentation’s repo, a feature-flag, or other static info about that library.

        The default included keys are the following:

            - ``name`` — name of the scope. Default value is model name and the entry point.

        More *typically* used keys might be as following (but not limited to and not included by default):

            - ``build.commit`` — commit hash of the build.
            - ``build.date`` — build date of the build.

``scheduler``
#############

    Type: **list**

    Scheduler is a component of Sysinspect Master, which is responsible for
    scheduling the *repetitive* tasks to call the minions. The aggregate *"scheduler"*
    takes a list of tasks. Each task is a list of key/value pairs:

    - ``name`` — name of the task. Type: **string**. This is a human-readable name of the task. It is used for logging purposes and should be unique.
    - ``query`` — query to be executed on the minion. Type: **string**. Query is written in a semicolon-separated format
        sending the following information:
        - model name
        - target scope (e.g. ``*`` for all targets)
    - ``traits`` — list of traits to be used for the query. Type: **string**. E.g. ``system.os.name:Ubuntu``.
    - ``interval`` — interval of the task, i.e. how often the task should be executed.
      This value can be in seconds, minutes or hours.
    - ``interval.unit`` — unit of the interval. Type: **string**. This value can be one of the following:

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

    Type: **string**

    Typically, Minion if running standard, the root of all data kept by a Minion is
    defaulted to ``/etc/sysinspect``, same as Master. However, in an embedded and custom
    systems this might not be possible, especially if the system is usually read-only
    and writable directories are limited to only a few. In this case *root* must be
    set according to the system setup.

``path.id``
###########

    Type: **string**

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

    Type: **string**

    The location of sharelib directory, which is by default is at the location
    ``/usr/share/sysinspect``. On most embedded systems those root filesystem is usually read-only,
    this location can be changed. This directory contains ``lib`` and ``modules`` subdirectories.

``path.tmp``
################

    Type: **string**

    The location of temporary directory, which is by default is at the location
    ``/var/tmp/sysinspect``. On most embedded systems those root filesystem is usually read-only,
    this location can be changed. This directory is used to store temporary files, such as downloaded modules,
    temporary data etc.

``master.ip``
#############

    Corresponds to ``bind.ip`` of Master node and should be identical.

``master.port``
###############

    Type: **integer**

    Corresponds to ``bind.ip.port`` of Master node and should be identical. By default it is
    set to ``4200``.

``master.fileserver.port``
##########################

    Type: **integer**

    Port of Master's fileserver. By default it is set to ``4201``.

``master.reconnect``
####################

    Type: **boolean**

    Sets reconnection to the master (or not). This is a boolean value, which is set to ``true`` by default.

``master.reconnect.freq``
#########################

    Type: **integer**

    Sets the frequency of reconnection to the master. This is a number of times, which is set to ``0`` by default.
    There are two options:

        - ``0`` — infinite reconnection attempts
        - ``n`` — number of reconnection attempts. If the number is reached, the minion will stop trying to reconnect.

``master.reconnect.interval``
#############################

    Type: **string**

    Interval (seconds) between reconnection attempts. This is a number of seconds, which is set to ``5-30`` range by default.
    Possible values are *(seconds, between the reconnection attemps)*:

        - ``n`` — specific amount of seconds
        - ``n-n1`` — a range of randomly selected seconds within that range

``modules.autosync``
####################

    Type: **string**

    Modules are always automatically synchronised at Minion boot. However, it requires full recalculation
    of each module's SHA256 checksum and it might take a while, if you have a lot of modules and they are big.
    Think of this as the *startup safety check* for modules:

    - it makes sure the minion has the modules it needs
    - it optionally verifies them by hashing (SHA256)

    More checking = more boot time. Less checking = faster boot, but you trust that files are unchanged.

    This value has the following options:

        - ``full`` — safest. Re-hash every module on every boot. Slowest, but detects any change.
        - ``fast`` — balanced. Use cached hashes when available; hash only what is missing. Good for most setups.
        - ``shallow`` — fastest. Only checks that module files exist (no hashing). Best for read-only/embedded boxes.
          Downside: it will not detect tampering or unexpected edits.

    Default is ``full``.

    Rule of thumb:

    - Shared server / security-sensitive: ``full``
    - Regular servers with many modules: ``fast``
    - Read-only image / tiny devices: ``shallow``

``log.forward``
##################

    Type: **boolean**

    Forward logs from actions and modules to the main sysinspect log, landing them in the main log file.
    If disabled, logs from actions and modules will not be forwarded to the main sysinspect log but are kept
    within their own context inside the returned data and will travel across the whole network back to the master.

    Thus, disabling this feature on a large cluster might inflate your network traffic so much that your network
    admin will start believe in ghosts and aliens.

    .. warning::

        Disable this option only if you really know what you are doing.

    Default is ``true``


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

    Type: **string**

    Directory, containing custom trait functions. They are meant to be defined on the Master side
    and then sync'ed to all the minions.

Only on Master
^^^^^^^^^^^^^^

Public and private RSA keys of Master are:

``$SR/master.rsa``
##################

    Type: **string**

    Master's private RSA key.

``$SR/master.rsa.pub``
######################

    Type: **string**

    Master's public RSA key.

``$SR/minion-keys``
###################

    Type: **string**

    Public keys from registered minions in format ``<minion-id>.rsa.pub``.

    Each registered minion has its own Id. Typically it is ``/etc/machine-id`` or automatically
    generated one, if this file does not exist.

``$SR/minion-registry``
#######################

    Type: **string**

    A binary cache of minion's data, such as minion traits, data about currently connected minions etc.
    This is fully purge-able directory, i.e. data can be freely deleted. However, Sysinspect Master
    needs to be restarted and all minions needs to reconnect.

Only on Minion
^^^^^^^^^^^^^^

Public and private RSA keys of Master are:

``$SR/master.rsa``
##################

    Type: **string**

    Minion's private RSA key.

``$SR/master.rsa.pub``
######################

    Type: **string**

    Minion's public RSA key.

``$SR/traits``
##############

    Type: **string**

    Directory, containing custom static traits of a Minion.

``$SR/models``
##############

    Type: **string**

    Directory, containing models.
