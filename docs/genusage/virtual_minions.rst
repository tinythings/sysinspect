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

*This feature is in early development and is subject to change in future releases.*

Why?
----

Unlike traditional configuration management systems, SysInspect is mainly an event-driven task launcher. It doesn’t try
to provide thousands of modules; instead, it focuses on a small set of simple primitives that reliably run workloads
based on a defined model.

Overview
--------

Clustered virtual minions are useful when you have several physical minions that should behave like one logical unit.
Instead of changing the configuration or state of the virtual minion itself, you use it as a single control point to
run functions on the underlying physical minions. Those functions are meant to act on external systems or services,
for example to run orchestration workflows, kick off monitoring or data-collection jobs, or trigger other automation
outside of the minions that back the cluster.

.. important::

  🚨 Virtual clustered minions are not designed or meant to manage or change *their own configuration or state*.
  They are primarily used to perform actions and/or launching workloads that are affecting other external systems.

  For example, running jobs, collecting metrics, orchestrating tasks on **other systems**, etc — depend on a module's
  capabilities that are launched on behalf of the virtual minion.

Minions can be grouped into logical collections called clusters. A cluster is simply a set of minions that share a
similar role, configuration, and model description, so you can treat them as a single unit instead of dealing with
each one separately.

From the outside, a cluster behaves like a single "virtual minion." Rather than talking to every physical minion on
its own, you talk to the cluster, and the cluster fans work out to the underlying machines. For example, if you have
several minions doing log analysis, you can group them into a cluster called "log-analyser" and assign those minions
to it. The cluster then exposes the modules, configuration, and model needed for log analysis in one place.

When you run a function against a cluster, SysInspect can either execute it on all member minions or choose one of
them (for example, the least busy node) to handle the job. This helps balance workloads and reduces the need to
manually pick which minion should do what, while still giving you a single, stable target to call.

The configuration example below shows how to define these "clustered minions" using a YAML structure. Each virtual
minion is described by a unique `id` and a `hostname`, which act as labels for grouping and identification. You can
also attach custom `traits` to each virtual minion, so you can target or filter them later based on those attributes.

Caveats and Considerations
--------------------------

- A virtual minion is only as reliable as the real machines behind it. If some of them are offline or misbehaving, the
  virtual minion will also work at reduced capacity or fail altogether. Make sure to monitor the health of the physical minions.

- All physical minions in one virtual minion must have the same modules installed and configured. Think of it like a
  shared Python virtualenv: if one minion is missing a module or has it misconfigured, you will get weird failures or
  hard-to-explain differences in behavior when calling the same function via the virtual minion.

  .. note::

    ⚠️ All minions that belong to a given virtual minion must have the same set of modules installed and configured.


Invocation
----------

Virtual minions are invoked with a different query syntax than regular minions. When you call all minions with
a `*` glob (or any kind of globbing), virtual minions are skipped. To call a virtual minion, you need to use
a ``v:`` prefix in the query, followed by the virtual minion hostname or glob pattern. For example:

.. code-block:: bash

    sysinspect your/model 'v:*'

Traits, however, remain the same, because `v:*` simply expands to all actual minions that back the virtual minion,
where the traits query will filter them further. For example, if your cluster has four minions, but two of them are
Ubuntu Linux and the others are FreeBSD, you can call only the Linux ones like this:

.. code-block:: bash

    sysinspect your/model 'v:*' -t 'system.os.name:Ubuntu'

In this case, the virtual minion expands to all physical minions, but the trait filter narrows it down to just
the Ubuntu ones.

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

Configuration starts with the `cluster` key, which contains a list of virtual minion definitions. Each :bi:`virtual minion` is defined
as a dictionary with the following keys:

  - :bi:`id`: A unique identifier for the virtual minion. Typically, this could be a UUID or any other unique string.

  - :bi:`hostname`: The hostname for the virtual minion.

  - :bi:`traits`: A dictionary of traits that can be used to target the virtual minion. Since virtual minions aren't physical, they can have only static traits, defined in this dictionary.

  - :bi:`nodes`: A list of physical minion matches.

Once this is done, each match can be then defined in one of the following ways:

    :bi:`id`

      A specific physical minion ID (e.g., `/etc/machine-id`).
      The `id` is dead-precise and matches the exact minion. In this case, no more qualifiers are needed.
      Just add all the minion IDs you want to be part of this virtual minion and that's it.

    :bi:`hostname`

      A specific hostname to match or glob pattern. This allows you to include physical minions based on their network identity.
      This is useful when you want to group minions by their hostnames, such as all minions in a certain domain or
      with a specific naming convention.

    :bi:`query`

      A query string that matches multiple physical minions (e.g., domain name patterns).

    :bi:`traits`
      A dictionary of traits that must be matched by the physical minion.


Within a single node definition, the different selector fields (:bi:`id`, :bi:`hostname`, :bi:`query`, and
:bi:`traits`) act as alternatives. A physical minion will be associated with the node if it matches any one of the
specified selectors — i.e., providing multiple selector fields widens the set of matched minions (logical OR across
selector types). However, the :bi:`traits` field itself is a collection of trait constraints that are evaluated against a
minion's traits; a trait constraint matches only when the minion satisfies the trait key/value pair(s) listed there.
Therefore if :bi:`traits` do not match any of the physical minions selected by the other selectors, no minion will be
associated with that node.

TL;DR:

.. code-block:: text

  (id + hostname + query)  ||  traits

.. hint::

  Always **keep it simple**!

  While you *can* define complex matching criteria, it doesn't mean you *should* do that.
  It's often best to start with straightforward configurations using just the :bi:`id` or the exact :bi:`hostname` and
  then expand as needed in a future. If you will use complex queries and trait filters, make sure to test and validate
  your configuration to avoid unexpected grouping results.

.. warning::

  🚨 Traits, when used in the call query, are matched against the :bi:`virtual minions`. Physical minions are guarded
  and closed by the virtual minion definition itself, therefore ``--traits`` option does not affect the actual physical minion selector.


.. code-block:: yaml

    # Example configuration for clustered minions

    cluster:
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

          # Matches by the hostname
          hostname: minion-01.example.com

          # This is where you can easily screw it all up
          query: "minion-*.example.com"

          # Matches all minions that are OS linux as well as system memory greater than 8Gb
          traits:
            system.os: "linux"
            system.mem: "> 8Gb"

