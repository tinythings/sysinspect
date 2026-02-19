**Pipeline**: Routing Events into Action/Model Calls
====================================================

.. note::

    This document explains how to use the **pipeline** event handler.

Overview
--------

The *pipeline* handler lets you "wire" actions and models together into a simple workflow. One action runs, SysInspect
emits an event with the result, and the pipeline can react to that event by triggering the next action/model.

In plain terms: you can take output from step A (JSON in the event), pick the bits you care about, and pass them as
context into step B. This is handy when you want automation that looks like a small runbook: check something, decide,
then fix it.

For example, a basic "self-healing" setup usually has at least two pieces:

1. **Inquiry** model: checks a service (or host) and returns useful data (CPU, memory, disk, HTTP status, version, etc.).
2. **Remediation** model: uses that data to decide what to do (restart, clear cache, reconfigure, scale, notify, etc.).

You can chain more steps if you want (e.g., validate the fix, then open a ticket if it still fails).

.. important::

  🚨 In SysInspect, assertions are events too, and they are emitted alongside action results. So for a single action you may
  receive multiple related events: one for the action return code/output, and separate ones for each assertion outcome.
  Your pipeline can react to either kind of event, depending on how you set up your event filters.

How It Works
------------

When an action finishes, SysInspect emits an event that contains the action result (return code, stdout/stderr, and
any structured ``data`` the action produced). The *pipeline* handler listens for events that match your event filter.

Most pipelines are wired to successful runs only, by using the ``...|0`` return-code filter (``0`` = success). With that
setup, a failed action (non-zero return code) simply will not match the filter, so the pipeline won't run for it. You'll
still see the failure in logs, and you can add a separate event filter if you also want to react to failures.

To enable the pipeline handler, add it to your configuration under the ``events`` section:

.. code-block:: yaml
    :caption: Initialisation

    handlers:
      - pipeline


From there, the handler will execute the ``calls`` you define for matching events, passing context along from one step
to the next (for example by pulling fields from the event using JSONPath).

Setup
-----

``calls``
^^^^^^^^^^

    This is where you define the sequence of actions or models you want to trigger. Each call can specify a query
    and an optional context. For example:

    .. code-block:: yaml
        :caption: Calls definition

        calls:
          - query: some/model
            context:
              key1: value1
              key2: value2

``query``
#########

    This is the query for the action or model you want to trigger. It can be a specific action
    or a model with a wildcard. For example:

    .. code-block:: yaml
        :caption: Query definition

        query: some/model # or "some/model/*" for all actions in that model

``context``
###########

    This is the context you want to pass to the next action or model. You can use JSONPath to extract values from the
    current action's output. For example:

    .. code-block:: yaml
        :caption: Context definition

        context:
          data: $.something # This will pass the value of $.something from the current
                            # action's output to the context of the next action

    Jinja syntax is also supported, so you can do more complex transformations if needed. For example:

    .. code-block:: yaml
        :caption: Jinja context example

        context:
          # This will pass a string "Hello, <value of $.yourname>" to the context of the next action
          data: "Hello, {{ $.yourname }}"

Example
-------

Routing events to the pipeline handler is basically "if this action says X, then run Y".

For example, imagine you have an inquiry action that checks something (a service, a config key, a
package version, free disk, whatever). If it finds a problem, it returns structured output in the
event's ``data`` payload.

Say the action returns something like this in ``data``::

  {"something": 500}

That means you can take the value (``500``), feed it into the next step as context, and then call a
remediation action/model that knows what to do with it.

This is the same idea as running SysInspect by hand and passing the value as a context parameter:

.. code-block:: bash

    sysinspect some/model '*' --context data:500

In the pipeline handler, you can do the same by defining the event filter and the context to pass,
except you read the value from the current event using JSONPath (for example, ``$.something``).

In other words:

* Your first action runs and emits an event.
* The pipeline matches that event (usually only successes, e.g. ``.../0``).
* The pipeline pulls fields out of the event payload with JSONPath.
* Those extracted values become the context for the next call.

So instead of hard-coding ``data:500``, you map it from the event like ``data: $.something``.

.. code-block:: yaml
    :caption: Setup example

    events:
      # Only react to successful actions (return code 0)
      $|$|$|0:

        handlers:
          - pipeline
        pipeline:
          # Optional, set to true to enable verbose logging
          verbose: false

          # Multiple calls can be defined here, they will be executed sequentially
          # as you wrote them in this configuration
          calls:
            - query: single/restore
              # Optionally, pass a context to the next model
              context:
                # This will pass the value of $.something from the current
                # action's output to the context of the next action
                data: $.something

