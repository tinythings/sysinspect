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

.. _mod_mgmt_tutorial:

Module and Library Management
=============================

.. note::

    This tutorial shows how to manage modules and libraries in a Sysinspect cluster.

Overview
--------

Modules are the main parts of Sysinspect's checks. They are stored in a shared repository
that all Minions in the Sysinspect cluster can access. Modules can be written in any language
as long as they follow the JSON format for input and output. Python modules are supported
but may need extra libraries.

Minions run the modules, and their output is sent back to the Master for processing.

Adding a Module
---------------

Assuming, there is already a module compiled, tested and working, it can be now published
to the shared repository at Master. To add a module to the shared repository, call the following
command on the Master:

.. code-block:: bash

    sysinspect module --add --name "your.module" -p /path/to/your/module --descr "My great module"

You will see a similar output, like this:

.. code-block:: text

    [28/01/2025 13:36:44] - INFO: Processing modules in /etc/sysinspect/data/repo
    [28/01/2025 13:36:44] - INFO: Adding module /path/to/your/module
    [28/01/2025 13:36:44] - INFO: Architecture: x86_64 ELF
    [28/01/2025 13:36:44] - INFO: Platform: linux
    [28/01/2025 13:36:48] - INFO: Module your.module added successfully

At this point, the module is available to all Minions in the cluster, but it is not yet
enabled. To enable the module, you would need to refresh/sync the entire cluster.

.. note::

    Binary modules are examined for their architecture and platform, and then placed accordingly.
    The minion then will pick up the correct module for its architecture and platform automatically.

    Python modules are considered as *scripts*, so they are outside of platform and architecture
    considerations, and therefore are available to all Minions at the same time.

Binary modules are examined for their architecture and platform, and then exported only to the
systems that match the module's architecture and platform.

.. warning::

    A binary module **must** be compiled for the correct architecture and platform. If the module is not
    compatible with the Minion's architecture and platform, it will not be found by a Minion.

Listing Available Modules
-------------------------

To list all available modules in the shared repository, call the following command on the Master:

.. code-block:: bash

    sysinspect module --list

You will see a similar output, like this:

.. code-block:: text

    linux (x86_64):
        fs.file  descr: Working with files
                  type: binary

        sys.run  descr: Run an application, raw.
                  type: binary

Removing a Module
-----------------

To remove a module from the shared repository, call the following command on the Master:

.. code-block:: bash

    sysinspect module --remove --name your.module

In order to remove more modules at once, you can list them separated by a comma:

.. code-block:: bash

    sysinspect module --remove --name your.module1,your.module2,your.module3


Adding a Library
----------------

Libraries are the additional dependencies that a module may require. They are stored in a shared
repository that all Minions in the Sysinspect cluster can access. To add a library to the shared
repository, call the following command on the Master:

.. code-block:: bash

    sysinspect library --add --library --path /path/to/your/library

You will see a similar output, like this:

.. code-block:: text

    [28/01/2025 13:36:44] - INFO: Processing library in /etc/sysinspect/data/repo
    [28/01/2025 13:36:44] - INFO: Copying library from /path/to/your/library
    [28/01/2025 13:36:44] - INFO: Library /path/to/your/library added to index

Unlike modules, library can be only one. So adding more files to it will overwrite the previous files
if they are the same name, or add new files if they are different.

Synchronising the Cluster
-------------------------

Although every Minion will automatically synchronise with the Master while starting, it is possible
to force the synchronisation while it is already running without restarting the Minion.

As modules and libraries are added to the shared repository, they are not immediately available to
the Minions. To make them available, the cluster must be synchronised. To synchronise the cluster,
call the following command on the Master:

.. code-block:: bash

    sysinspect --sync

At this point all Minions will synchronise with the Master and download the new modules and libraries,
removing or replacing the old ones. Once this is done, the new modules and libraries are available
to the Minions immediately and can process the data accordingly.