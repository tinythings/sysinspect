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

Communication Protocol
======================

.. note::

    Protocol description in JSON format

Part of the communication protocol is already tied up to the :ref:`modstruct`. It is still
left on receiving the data.

Formatting Request
------------------

As previously mentioned, the request must be strictly aligned with the structure of the :ref:`modindex`, but resembling only the ``arguments`` and ``options`` structures.

The following example shows how a module defines the arguments and options, as well as
it would expect as an input:

.. code-block:: yaml
    :caption: Module Documentation

    arguments:
      - name: "mask"
        type: "string"
        required: true
        description: "Globbing mask"

    options:
      - name: "verbose"
        description: "Provide verbose output"

.. code-block:: json
    :caption: Expected JSON input

    {
        "arguments": [
            {"mask": "*.txt"},
        ],
        "options": ["verbose",],
    }

Additionally, ``data`` key/value container is used for *arbitrary* data. This can be
additional facts, constraint expressions or anything that is possible be relevant to
the particular module. This section is understood only by this particular module and
is just a static container of anything. Example:

.. code-block:: json
    :caption: Payload example

    {
        // Just for the reference
        "arguments": [],
        "options": []

        // Payload data
        "data": {
            "some-key": ["what", "ever", "data",],
        },
    }

Formatting Response
-------------------

Module should always be compliant on response to the SysInspect, so its data is properly
received and processed. Any "junk" data in JSON will be just ignored. If any of the required
fields are missing, then the response from the module is marked as invalid.

The following response fields are supported:

``retcode``

    Type: ``Integer``. This is the return error code. Anything other than ``0`` is an error.

``warning``

    Type: ``List`` containing records of type ``String``. For example, a task can return ``retcode``
    as ``0`` (success), however some warnings appeared on the way. They can be returned alongside.

``message``

    Type ``String``. The main return message. It is either an error message or success message.
    It should not contain anything with regard to the warning or any other off-topic.

``data``

    It is any kind of data in any structure. Example:

    .. code-block:: json
        :caption: Tabular data

        {
            "data": [
                ["process", "uid", "pid"],
                ["dpkg", "root", 1885],
                ["/usr/libexec/fwupd/fwupd", "root", 4055],
                ["/usr/libexec/upowerd", "root", 3137],
            ],
        }

    Obviously, it can be a compound response, which the formatter on the receiving side must
    understand as well *(the* :bi:`data/processes` *path to the table or* :bi:`data/uptime` *path to
    the system general uptime data etc)*.

    .. code-block:: json
        :caption: Compound data

        {
            "data": {
                "processes": [
                    ["process", "uid", "pid"],
                    ["dpkg", "root", 1885],
                    ["/usr/libexec/fwupd/fwupd", "root", 4055],
                    ["/usr/libexec/upowerd", "root", 3137],
                ],
                "uptime": [4154595.94, 81372980.25],
            },
        }

    .. caution::

        While data is literally *any structure*, however keep in mind that it at some point
        it must be *somehow* understood on the receiver side. Typically, action must define
        the data formatter. It might be a table, or key/value structure, or just a string,
        or an array of integers etc.
