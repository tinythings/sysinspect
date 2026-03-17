.. _profiles_tutorial:

Deployment Profiles Tutorial
============================

.. note::

    This tutorial walks through the full deployment profile flow on the
    Master side: create profiles, add selectors, assign profiles to
    minions, sync, and verify the result.

Overview
--------

Deployment profiles define which modules and libraries a minion is allowed
to sync.

Profiles are:

* defined on the Master
* stored one profile per file
* assigned to minions through the ``minion.profile`` static trait
* enforced during minion sync

The effective result is:

1. a minion resolves its assigned profile names
2. it downloads the corresponding profile files
3. it merges selectors by union + dedup
4. it syncs only the allowed modules and libraries

What We Will Build
------------------

In this tutorial we will:

1. create a narrow profile named ``tiny-lua``
2. allow only the Lua runtime and Lua-side libraries
3. assign that profile to one or more minions
4. sync the cluster
5. verify that only the allowed artefacts are present

Prerequisites
-------------

This tutorial assumes:

* the Master is running
* one or more minions are already registered
* the runtime modules and libraries are already published in the module repository

If you need the repository basics first, see :ref:`mod_mgmt_tutorial`.

Creating a Profile
------------------

Create a new profile on the Master:

.. code-block:: bash

    sysinspect profile --new --name tiny-lua

This creates a profile entry in ``profiles.index`` and a profile file under
the Master's profiles directory.

List available profiles:

.. code-block:: bash

    sysinspect profile --list

Expected output should include:

.. code-block:: text

    tiny-lua

Adding Module Selectors
-----------------------

Now add the module selectors allowed by this profile:

.. code-block:: bash

    sysinspect profile -A --name tiny-lua --match "runtime.lua"

List the module selectors:

.. code-block:: bash

    sysinspect profile --list --name tiny-lua

Expected output:

.. code-block:: text

    tiny-lua: runtime.lua

Show the fully expanded profile content as a mixed modules/libraries table:

.. code-block:: bash

    sysinspect profile --show --name tiny-lua

Adding Library Selectors
------------------------

Add the library selectors allowed by the same profile:

.. code-block:: bash

    sysinspect profile -A --lib --name tiny-lua --match "lib/runtime/lua/*.lua,lib/sensors/lua/*.lua"

List the library selectors:

.. code-block:: bash

    sysinspect profile --list --name tiny-lua --lib

Expected output:

.. code-block:: text

    tiny-lua: lib/runtime/lua/*.lua
    tiny-lua: lib/sensors/lua/*.lua

Editing a Profile
-----------------

If you added a selector by mistake, remove it with ``-R``:

.. code-block:: bash

    sysinspect profile -R --name tiny-lua --match "lib/sensors/lua/*.lua" --lib

You can then add the correct selector again:

.. code-block:: bash

    sysinspect profile -A --lib --name tiny-lua --match "lib/sensors/lua/*.lua"

Profile files have a canonical ``name`` inside the file. The filename is
only storage.

New profiles are written with lowercase filenames by default, but the index
can still point at any existing filename and that filename will continue to
work unchanged.

For example, a profile file looks like this:

.. code-block:: yaml

    name: tiny-lua
    modules:
      - runtime.lua
    libraries:
      - lib/runtime/lua/*.lua
      - lib/sensors/lua/*.lua

Assigning a Profile to Minions
------------------------------

Assign the profile to minions by query:

.. code-block:: bash

    sysinspect profile --tag "tiny-lua" --query "pi*"

Assign the profile to one exact minion by System Id:

.. code-block:: bash

    sysinspect profile --tag "tiny-lua" --id 30006546535e428aba0a0caa6712e225

You can also combine profile assignment with trait-based minion targeting:

.. code-block:: bash

    sysinspect profile --tag "tiny-lua" --traits "system.os.name:NetBSD"

This updates the master-managed ``minion.profile`` static trait on the
targeted minions.

Removing a Profile Assignment
-----------------------------

To remove one assigned profile from targeted minions:

.. code-block:: bash

    sysinspect profile --untag "tiny-lua" --query "pi*"

If all assigned profiles are removed, the minion falls back to the
``default`` profile during sync.

``default`` is fallback-only. It is not stored together with real assigned
profiles.

Synchronising the Cluster
-------------------------

After creating or changing profiles, refresh the cluster:

.. code-block:: bash

    sysinspect --sync

During sync:

1. minions download ``mod.index``
2. minions download ``profiles.index``
3. minions resolve ``minion.profile``
4. minions download the selected profile files
5. minions merge all selectors by union + dedup
6. minions sync only the allowed artefacts

On minion startup, Sysinspect also logs the effective profile names being
activated.

Verifying the Result
--------------------

There are a few practical ways to verify the setup.

Check the assigned profile on the Master:

.. code-block:: bash

    sysinspect --ui

The TUI can be used to inspect online minions and their traits, including
``minion.profile``.

Check the profile definition on the Master:

.. code-block:: bash

    sysinspect profile --list --name tiny-lua
    sysinspect profile --list --name tiny-lua --lib
    sysinspect profile --show --name tiny-lua

Check the minion logs:

.. code-block:: text

    INFO: Activating profile tiny-lua

or, for multiple profiles:

.. code-block:: text

    INFO: Activating profiles tiny-lua, runtime-full

Using the Shipped Examples
--------------------------

Sysinspect ships example profile files under:

* ``examples/profiles/tiny-lua.profile``
* ``examples/profiles/runtime-full.profile``

They are examples of the exact profile file format accepted by the Master.

Deleting a Profile
------------------

When a profile is no longer needed:

.. code-block:: bash

    sysinspect profile --delete --name tiny-lua

This removes both:

* the ``profiles.index`` entry
* the stored profile file on the Master

Summary
-------

The profile workflow is:

1. create a profile
2. add module selectors
3. add library selectors
4. assign the profile to minions
5. sync the cluster
6. verify the result

That is the complete deployment profile loop in Sysinspect.
