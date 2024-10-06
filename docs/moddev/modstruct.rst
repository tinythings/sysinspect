.. _modstruct:

Module Structure
================

.. note::

    Description of the Module structure and basic requirements


Module Documentation
--------------------

Module documentation is the protocol description for the module.
Typically it is a YAML file with the name ``mod_doc.yaml`` in the root of the
module source tree. It contains various attributes about the module itself
and description of options and arguments it receives. It also contains example
usage. This document is used to generate help page for the module.

.. warning::

    Content of the Module Documentation content is always a subject to change!

Below are currently supported and required sections and attributes:

``name``
^^^^^^^^

    Name of the module, contains module namespace, which is also its unique id.
    Type ``String``, **required**. Example:

    .. code-block:: yaml

        name: "sys.proc"

``version``
^^^^^^^^^^^

    Version of the module in `semantic versioning format <https://semver.org>`_.
    It is displayed on the help, also can be requested by the caller separately
    for backward-compatibility purposes. Type ``String``, **required**. Example:

    .. code-block:: yaml

        version: "0.0.1"

``author``
^^^^^^^^^^

    Name of the author of the module. Type ``String``, **required**. Example:

    .. code-block:: yaml

        author: "Bo Maryniuk"

``description``
^^^^^^^^^^^^^^^

    Description of the module. Multiline text allowed. It will be reformatted
    with 80 characters width. Type ``String``, **required**. Example:

    .. code-block:: yaml

        description: |
          Very cool plugin for storage examination, to makes your life easier.
          It is also written in Perl, to make your bigfixing life harder. :)

``options``
^^^^^^^^^^^

    List of options. Those are like typical CLI flags without any values
    and storing only boolean types. Type ``List``, optional. Each option
    is an element in a list of key/value mapping. Each keyword is required.
    Below is the list of supported keywords:

    ``name``

        Name of an option. Type ``String``, **required**.

    ``description``

        Description of an option. Type ``String``, **required**.

        .. note::

            Description can be multiline, but it is encouraged to be as sparse as possible,
            usually fitting to just one line.

    Example of two defined options:

    .. code-block:: yaml

        options:
          - name: "verbose"
            description: "Provide verbose output"

          - name: "follow-symlinks"
            description: "Follow symlinks while walking the directory tree"

``arguments``
^^^^^^^^^^^^^

    List of keyword arguments. A keyword argument is a flag with the value or several values.
    Those are equivalent to args in CLI, those are typically formatted as key/value, e.g.: ``--foo=bar``.
    Each argument is an element in a list of key/value mapping. Not all keywords are required.
    Below is the list of supported keywords:

    ``name``

        Name of an option. Type ``String``, **required**.

    ``description``

        Description of an option. Type ``String``, **required**.

        .. note::

            Description can be multiline, but it is encouraged to be as sparse as possible,
            usually fitting to just one line.

    ``type``

        Type of an argument. Types are yielding YAML supported types: string, bool, int, list etc.
        However, typically there are three types preferred: *string*, *bool* and *integer*. Field
        type is ``String``, **required**.

    ``required``

        Flag, setting the argument required to be passed on or not. Type ``Bool``, **required**.

    ``default``

        Default value for the argument, if the field ``required`` is set to ``false``. Type is corresponding
        to the value and can be one of ``String``, ``Bool``, ``Int``, ``List``, ``Mapping`` etc, optional.

    Example of a defined argument:

    .. code-block:: yaml

        argument:
          - name: "directory"
            type: "string"
            default: "/tmp"
            required: false
            description: "Directory where to store something"

``examples``
^^^^^^^^^^^^

    Adding examples of module usage is a very good practice, it gives quick grasp for newcomers,
    as well as reminds experienced users how to use the module. It is encouraged to be generous
    on examples, but keep it sane.

    This section has only two keywords: ``description`` and ``code``.

    ``description``

        A multiline description what the example is about. Type ``String``, **required**.

    ``code``

        An actual multiline example of a protocol in a JSON format. Type ``String``, **required**.
