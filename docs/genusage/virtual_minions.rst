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

Clusters and Virtual Minions
============================

Minions can be organized into groups called clusters. A cluster is a collection of minions that share a common
configuration and model description. This allows you to manage multiple minions as a single unit.

A cluster acts like a "virtual minion." Instead of interacting with each minion individually, you can interact with
the cluster as a whole. For example, if you have several minions that are responsible for log analysis, you can create
a cluster named "log analyser" and assign those minions to it. The cluster will have all the necessary modules and
settings for log analysis.

When you run a function on a cluster, the system can either use all minions in the cluster or select the one that is
least busy to perform the task. This makes it easier to manage workloads and ensures that tasks are distributed
efficiently among the available minions.
This configuration example demonstrates how to define "clustered minions" using a YAML structure. Each virtual minion is
described by a unique `id` and a `hostname`, which serve as labels for identification and grouping purposes. You can
assign custom `traits` to each virtual minion, allowing you to specify characteristics or metadata that can be used for
targeting or filtering.

Virtual Minion Definition
--------------------------------

The `nodes` section under each virtual minion defines how physical minions are matched and associated with the virtual
minion. There are several ways to specify these matches:

  - By unique physical minion ID (e.g., `/etc/machine-id`), allowing precise targeting of individual machines.

  - By query patterns (such as domain names with wildcards), enabling selection of groups of minions based on naming
    conventions.

  - By specifying required traits (e.g., operating system type, memory size), which allows for dynamic selection based on
    system properties.

  - By combining queries and trait filters, you can create complex selection criteria, such as targeting all minions with a
    certain name prefix and a minimum amount of memory.

This flexible configuration enables you to create logical groupings of physical minions, assign them virtual identities,
and target them for orchestration, monitoring, or other management tasks based on a wide range of criteria.

.. code-block:: yaml

    # Example configuration for clustered minions

    clustered-minions:
    # Each minion has a virtual ID and virtual hostname
    # These are basically just labels
    - id: 12345
      hostname: fustercluck
      # Virtual traits by which virtual minions are targeted
      traits:
      key: value

      # Physical minion matches
      nodes:
        # Matches a very specific minion by its /etc/machine-id
        - id: 30490239492034995

        # Matches all minions configured with domain name started with "web" prefix
        - query: "web*"

        # Matches all minions those are OS linux
        - traits:
            system.os = "linux"

        # Matches all minions configured with domain name started with "web" prefix,
        # but selects only those system memory is more than 8Gb RAM
        - query: "web*"
            traits:
            system.mem > 8Gb
