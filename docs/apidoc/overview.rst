Web API
=======

This section provides an overview of the Web API for SysInspect, including its
HTTPS/TLS access pattern, endpoints, and request flow.

Accessing the Documentation
---------------------------

The Web API is automatically documented using Swagger, which provides a
user-friendly interface to explore the available endpoints and their
parameters. You can access the Swagger UI at:

``https://<your-server-address>:4202/doc/``

This interface runs on default port **4202**.

The embedded Web API only starts when TLS is configured correctly under
``api.tls.*`` in ``sysinspect.conf``.

The documentation endpoints are controlled by ``api.doc``. When
``api.doc: false``, ``/doc/`` is not exposed.

Authentication And Requests
---------------------------

The Web API uses:

- HTTPS/TLS for transport protection
- bearer tokens for authentication
- plain JSON request and response bodies

Typical flow:

1. ``POST /api/v1/authenticate`` with JSON credentials
2. receive ``access_token``
3. call later endpoints with ``Authorization: Bearer <token>``

Example authentication request body:

.. code-block:: json

   {
     "username": "operator",
     "password": "secret"
   }

Example query request body:

.. code-block:: json

   {
     "model": "cm/file-ops",
     "query": "*",
     "traits": "",
     "mid": "",
     "context": {
       "reason": "manual-run"
     }
   }

Related Material
----------------

- :doc:`../global_config`
- :doc:`../genusage/operator_security`
- ``examples/transport-fixtures/webapi_auth_request.json``
- ``examples/transport-fixtures/webapi_query_request.json``
