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

.. _commproto:

Communication Protocol
======================

.. note::

    Protocol description in JSON format

Part of the communication protocol is already tied up to the :ref:`modstruct`. It is still
left on receiving the data.

Request Format
--------------

As previously mentioned, the request must be strictly aligned with the structure of the :ref:`modindex`.
For historical reasons, both ``arguments`` / ``options`` and ``args`` / ``opts`` are supported.
The former must remain supported; the latter are accepted aliases.

The top-level runtime request contract is:

.. code-block:: json

    {
        "arguments": {},
        "options": [],
        "config": {},
        "ext": {},
        "host": {}
    }

Equivalent alias form:

.. code-block:: json

    {
        "args": {},
        "opts": [],
        "config": {},
        "ext": {},
        "host": {}
    }

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
        "arguments": {
            "mask": "*.txt"
        },
        "options": ["verbose"],
        "config": {},
        "ext": {},
        "host": {}
    }

``config`` is the full runtime config payload. It remains available to runtimes as-is.

``host`` is descriptive only. Its primary facts surface is ``host.traits``, which is a serialized
map of minion traits. This is intentionally dynamic, because traits can be built from multiple
sources, including user-controlled ones. Small convenience sections such as ``host.paths`` and
``host.capabilities`` may exist, but they are secondary to ``host.traits``.

Helper Taxonomy
---------------

Runtime helpers are split into two categories:

* passive descriptive data
* active helper operations

Passive descriptive data stays in the shared request payload, primarily under ``host``.
This is the portable core contract and should be preferred whenever a script only needs
facts about the current minion or runtime call.

Active helper operations must stay in explicit helper namespaces instead of being stuffed
into ``host``. This keeps the protocol boundary clear: ``host`` describes the system,
while helper namespaces perform host-assisted operations.

Current portable helper surfaces are:

* ``host`` helper facades in Lua, Py3, and Wasm guest helper code
* runtime logging facilities, normalised into ``__sysinspect-module-logs``

Current platform-specific helpers are:

* ``packagekit`` for Lua and Py3
* low-level PackageKit host imports for Wasm guests

``packagekit`` remains intentionally separate from ``host``. It is an active, Linux-specific
integration and is not part of the portable descriptive contract.

``ext`` is used for *arbitrary* caller-specific data. It is understood only by the receiving module.
Example:

.. code-block:: json
    :caption: Payload example

    {
        "arguments": {},
        "options": [],
        "config": {},
        "host": {},

        "ext": {
            "some-key": ["what", "ever", "data",],
        }
    }

The contract intentionally excludes transport/session internals, side-effecting APIs, and unstable
telemetry blobs. Runtime-control names under ``rt.*`` remain reserved internal protocol parameters.

Migration Notes
---------------

Older SysInspect setups exposed useful host data through embedded-language
bindings. That model is gone.

Use these replacements instead:

* host facts from ``host.traits``
* runtime-relevant paths from ``host.paths``
* full minion/runtime configuration from ``config``
* portable helper sugar from the runtime ``host`` helpers in Lua, Py3, and Wasm

Native ``.py`` modules are no longer resolved by ``libsysinspect``. Python is
available only through the ``runtime.py3`` dispatcher and the virtual
``py3.<module>`` namespace.

.. hint::

    As keyword arguments are quite primitive and support only ``int``, ``string`` and ``bool``,
    one may still pass a list of strings as one comma-separated string. Example:

    .. code-block:: json

        "arguments": {
          "my-list": "one,two,three,four",
        }

    However, the burden of parsing this lays solely on module itself, therefore ``libsysinspect``
    already has specific instrumentation for this.


.. _formatting-response:

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
