``net.http``
============

.. note::

    This document describes ``net.http`` module usage.

Synopsis
--------

``net.http`` is a generic one-shot HTTP client module. It sends one outbound
request, returns a structured response, and then exits.

The implementation is in ``modules/net/http/src/http.rs``, where the request is
built from the module runtime arguments and executed through ``reqwest``
blocking client.

Action DSL setup
----------------

This module is called through the standard action DSL described in
``docs/modeldescr/actions.rst``. The action parser accepts:

* ``module`` for the module namespace,
* ``bind`` for bound entities,
* ``state`` for per-state module parameters,
* ``args`` or ``arguments`` for keyword arguments,
* ``opts`` or ``options`` for module options.

``net.http`` does not currently define any module options, so the request is
normally configured only through ``args``.

Example action:

.. code-block:: yaml

    actions:
      jira-search:
        module: net.http
        bind:
          - api-target
        state:
          $:
            args:
              method: GET
              url: https://jira.example.com/rest/api/2/search
              query:
                jql: project = OPS ORDER BY updated DESC
              headers:
                Accept: application/json
              auth:
                type: bearer
                token: context(jira_token)

The action layer serialises these values into the module request JSON on
stdin. That request shape is defined in ``libmodcore/src/runtime.rs``, where
``args`` is accepted as an alias of ``arguments`` and ``opts`` as an alias of
``options``.

Arguments
---------

``method`` (type: string, required)

    HTTP method. Parsed using ``reqwest::Method``.

``url`` (type: string, required)

    Target URL.

``headers`` (type: object)

    Request headers as a key/value object. Values must be scalar JSON values.

``query`` (type: object)

    Query-string parameters as a key/value object. Values must be scalar JSON
    values.

``body`` (type: any)

    Request body. Objects and arrays are sent as JSON. Strings are sent as raw
    text. Other scalar values are converted to text.

``auth`` (type: object)

    Authentication object. Supported values come directly from
    ``modules/net/http/src/http.rs``:

  * ``type: bearer`` with ``token``
  * ``type: basic`` with ``username`` and optional ``password``
  * ``type: header`` with ``header`` and ``value``
  * ``type: query`` with ``param`` and ``value``

``tls`` (type: object)

    TLS settings object. Supported fields:

  * ``ca_file``
  * ``client_cert_file``
  * ``client_key_file``
  * ``insecure_skip_verify``
  * ``insecure_skip_hostname_verify``

  If ``client_cert_file`` is set, ``client_key_file`` must also be set.

``timeout`` (type: int)

    Request timeout in seconds. Default is ``30``.

``ok-status`` (type: int or list)

    Extra HTTP status codes treated as success in addition to the normal 2xx
    range.

Examples
--------

Bearer token GET:

.. code-block:: json

    {
      "arguments": {
        "method": "GET",
        "url": "https://jira.example.com/rest/api/2/search",
        "query": {
          "jql": "project = OPS ORDER BY updated DESC"
        },
        "headers": {
          "Accept": "application/json"
        },
        "auth": {
          "type": "bearer",
          "token": "secret-token"
        }
      }
    }

POST JSON with custom CA trust:

.. code-block:: json

    {
      "arguments": {
        "method": "POST",
        "url": "https://processor.example.com/api/jobs",
        "body": {
          "ticket": "OPS-42",
          "artifact": "/data/log.bin"
        },
        "tls": {
          "ca_file": "/etc/ssl/private/processor-ca.pem"
        }
      }
    }

Return data
-----------

The module response structure is built in ``modules/net/http/src/http.rs``. The
``data`` section contains:

.. code-block:: json

    {
      "url": "https://api.example.com/resource",
      "status": 200,
      "ok": true,
      "headers": {
        "content-type": "application/json"
      },
      "body": {
        "text": "{\"ok\":true}",
        "json": {
          "ok": true
        }
      }
    }

If the response body is not UTF-8, ``body`` contains ``base64`` instead.

If the HTTP status is neither successful nor listed in ``ok-status``, the
module returns ``retcode: 1`` and message ``HTTP request failed with status
<code>``.
