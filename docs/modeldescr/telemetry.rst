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

Telemetry
=========

.. note::

    This document explains how to obtain and export telemetry data.

SysInspect is designed mainly to obtain and export telemetry data by :bi:`generating new metrics`,
watching the system and its components. The telemetry data is collected from the system
and its components, and then exported to a telemetry data sink. The telemetry data sink
is a place where telemetry data is stored and can be accessed by other components.

SysInspect is using the OpenTelemetry (OTLP) spec to collect and export telemetry data.

Configuration
-------------

Since SysInspect is using the OpenTelemetry (OTLP), it needs to be configured alongside
with the observability pipeline. The observability pipeline is a set of components
that collects, processes, browses or re-exports telemetry data.

.. important::

    Configuration of the end-point is located in :ref:`global_configuration` section,
    while the configuration of the telemetry data is located in the Model Description.

Typically, SysInspect needs an OTEL collector endpoint, where it can send telemetry data
using gRPC protocol. This is done in the SysInspect's Master configuration file.
Please refer to the :ref:`global_configuration` for more details, looking in particular for
``telemetry.collector.*`` options in ``Master`` section.

Telemetry data is configured in the Model Description.

Obtaining Data
--------------

Telemetry data is obtained by using the ``telemetry`` section in the Model Description.
There are two "entry points" where telemetry data can be obtained:

- **model** - this is called each time the Model is invoked. That is,
  you will receive telemetry data from the *previous* call of the Model.

- **minion** - this is called every time a minion returns data. SysInspect currently does not
  track or re-verify which minions were called or what data was returned from them, although
  it is essentially possible.

These entry points are basically the same, except that the ``model`` entry point has map/reduce
functionality to aggregate the data.

``minion``
^^^^^^^^^^

The ``minion`` entry point is used to obtain telemetry data from each minion. This means that
this selector will be called *for each minion*.

``select`` (optional, list)

    First, you need to define what minions to select and what to ignore for you specific situation.
    To do so, use ``select`` section in the Model Description, which is a list of selectors.
    Selector is basically either in a form of trait key-value pair or a Unix regular expression,
    applicable to the minion's hostname(s).

    Example:

    .. code-block:: yaml

      # Select minions by their traits
      select:
        - "system.os:Ubuntu"
        - "system.cpu:x86_64"

        # Select minions by their hostname
        - "web*.mydomain.com"

    At least one selector is required. If selector is omitted, it is default to ``"*"`` per hostnames (all).

``data`` (required, map)

    Data is usually a key/value structure, which is coming from the whatever module one wants to use in the
    current model. Since key/value return is unpredicable, user has to define the key and the path how to
    obtain that value. Depending on the type of the attribute, the data selector might be different. In JSON
    case, the selector will be a JSONPath expression.

    Here should be defined a key (which is anything) that will be filled with the value of
    the defined path or just ``null``. Then these keys can be optionally used in the export section
    in attr-format to format the data into a static string.

    Path starts with the operation label, and then the actual JSON path to the data. For example,
    if you called it over ``file-creation`` and the module is collecting date/time and filetype
    in some nested keys, then the path would be something like:

    Example:

    .. code-block:: yaml

      data:
        datetime: file-creation.file.created_at
        filename: file-creation.file.name

    This should internally produce a telemetry record like this:

    .. code-block:: json

        {
            "datetime": "2023-10-01T12:00:00Z",
            "filename": "/tmp/test.txt"
        }

``filter`` (optional, map)

    This is an optional section that can be used to filter-out the data that is obtained from the minion.
    It is a map of key/value pairs, where key is the name of the attribute and value is the value
    that should be matched. If the value is a list, then it will match any of the values in the list.

    It has the following keys to define:
    - ``entity`` — this is the name of the entity that will be filtered.
    - ``actions`` — this is a list of actions that will be filtered.

    Example:

    .. code-block:: yaml

      filter:
        entity: file-ops
        actions:
          - info-netconfig
          - info-groups

``export`` (required, map)

    At this point you need to define how the data should be exported. This is done by
    defining the ``export`` section. This section is a map of key/value pairs.

    It has the following keys to define:

    - ``attr-name`` — this is the name of the OTLP attribute/field that will be used to export the data.
      This is usually a string, but it can be anything. It is recommended to use a string
      that is unique and descriptive. For example, typically for logs it is called ``message``.

    - ``attr-type`` — this is the type of the attribute, into which the data needs to be serialised.
      By default is is ``json``.

    - ``attr-format`` — an optional :bi:`string format` of the attribute, based on the obtained key-value
      pairs. Using the example above, the format could be:

      .. code-block:: yaml

          attr-format: "The file {filename} was created at {datetime}."

    - ``data-type`` — this is the type of the data that will be used to export the obtained telemetry.
      Optionally, explicitly enforce the type of the data per a key. Note, that not all keys needs to be
      converted/casted to any other type.

      .. code-block:: yaml

        data-type:
            filename: string
            datetime: string

    - ``telemetry-type`` — this is the type of telemetry data that will be used to export the obtained.
      OpenTelemetry supports several types of telemetry data, such as logs, metrics and traces.
      SysInspect currently supports only logs (metrics are planned in a future releases),therefore default
      value currently is ``log``:

      .. code-block:: yaml

         telemetry-type: log # or metric, in a future

    - ``static-destination`` — destination where the telemetry data will be placed within a log message.
      Valid values are:

        - ``attributes`` — this is the default value. The telemetry data will be placed in the attributes
          section of the log message. Usually static data is not supposed to often change, so ``attributes``
          is a default place where to put it.
        - ``body`` — the telemetry data will be placed in the body of the log message.

        .. note::

          If the ``attr-type`` is set to ``string``, then there is a :bi:`potential data loss``
          when using ``body`` is a destination. This is because that the ``attr-format`` might
          not be able to interpolate all the data to a string (e.g. user forgot to take that field).

    - ``static`` — this is an optional static data that will be added to the telemetry record.
      This is usually used for markers, namespaces, tags etc. It is a map of key/value pairs.

        .. code-block:: yaml

          static:
              foo: "This is added statically"
              bar: 1234

        This data will be merged to the telemetry record, and will be available in the exported data "as is".


``model``
^^^^^^^^^

The ``model`` entry point is used to obtain telemetry data from the Model. It has all the attributes that
has ``minion`` entry point, except it adds map/reduce functionality to aggregate the data and generate one
metric per a model call, typically to average the result.

Map/reduce can be defined in the following sections:

``map``

    A function that will be applied to each value per a minion. Operations per key, per value.
    Each operation is a function that will be applied to the value. The result of the operation will be used
    as a value for the key, returns a :bi:`list of results`.
    Valid functions are:

    - ``round`` — rounds the value to the nearest integer.
    - ``to-int`` — converts the value to an integer.
    - ``to-float`` — converts the value to a float.
    - ``to-bool`` — converts the value to a boolean.
    - ``to-str`` — converts the value to a string.
    - ``less-than`` — checks if the value is less than the given value.
    - ``greater-than`` — checks if the value is greater than the given value.
    - ``equals`` — checks if the value is equal to the given value.

    Example:

    .. code-block:: yaml

      # The map function will be applied to each value per a minion.
      map:
        my-key: less-than 10
        my-other-key: greater-than 42

``reduce``

    A function that will be applied to the list of results, returned by map, returns a :bi:`single result`.
    Valid functions are:

    - ``sum`` — sums the values.
    - ``average`` — calculates the average of the values.
    - ``min`` — finds the minimum value.
    - ``max`` — finds the maximum value.
    - ``count`` — counts the number of values.

    Example:

    .. code-block:: yaml

      reduce:
        my-key: sum
        my-other-key: average
