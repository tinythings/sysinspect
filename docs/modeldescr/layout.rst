Layout and Setup
================

.. note::

    Introduction to the Model Description Setup


Model Description is a collection of YAML files within one directory that may contain sub-directories.

.. important::

    Layout requirements:

    1. Should be a subdirectory
    2. Should contain ``model.cfg`` in the root of it.

Essentially, a directory with a file ``model.cfg`` is the root of the model. All other directories,
filenames and their placement is completely free. At the end, model will be scanned and merged into
one YAML tree and then processed accordingly.

Example:

.. code-block:: text

    my_model/
            |
            +-- model.cfg
            |
            +-- actions/
            |          |
            |          +-- network-actions.cfg
            |          |
            |          +-- disk-actions.cfg
            |
            +-- constraints.cfg
            |
            +-- entities.cfg

The most important here is ``model.cfg``, which is equivalent to ``index.html`` in a static website.

``Model.cfg``
-------------

Model description index file has the following structure:

.. code-block:: yaml

    name: My Great Model
    version: "0.1"
    description: |
        This is a description of this model
        that gives you more idea what it is etc.
    maintainer: John Smith <john.smith@example.com>
    interface: null
    checkbook: null
    config: null

The following fields are supported:

``name``

   Name of the model.

``version``

   Model version.

``description``

   Multi-line model description.

``maintainer``

   Model maintainer (name, email)

``config``

    Global configuration section. It is applied to the whole session, globally. However
    different model can have a different configuration.

``interface``

   The ``interface`` section is optional. Its purpose is to define the public callable surface
   of the model explicitly, rather than relying on the full internal structure of the model.
   This is especially useful when a model contains helper entities or internal gating actions that
   should participate in evaluation, but should not be shown as operator-facing targets.

   The syntax is intentionally simple and typed. It accepts three optional lists named
   ``checkbook``, ``entities`` and ``actions``.

   .. code-block:: yaml

      interface:
        checkbook:
          - main-audit
        entities:
          - all
          - summary
        actions:
          - python-proof

   In this form, ``main-audit`` is a public checkbook label, ``all`` and ``summary`` are public
   entity entrypoints, and ``python-proof`` is declared as a public direct action entrypoint.
   The declaration itself does not change the internal execution graph of the model. It simply
   states what should be treated as public by tools that inspect the model.

   If ``interface`` is not present at all, SysInspect keeps the historic behaviour. In that case,
   every inferred entrypoint is considered public.

``checkbook``

   Checkbook is a list of sections that groups relations those needs to be checked.
   An example:

   .. code-block:: yaml

      checkbook:
        my_label:
          - relation-one
          - relation-two

        my_other_label:
          - relation-one
          - relation-three

   In this case user can call ``my_label`` and SysInspect will only go through relations,
   grouped inside that section, leaving all other untouched. If checkbook is omitted,
   then all relations will be examined, one after another.
