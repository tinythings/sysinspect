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
