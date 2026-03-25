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

If ``api.tls.ca-file`` is configured, the same mutual-TLS requirement applies
to Swagger UI and the OpenAPI JSON endpoint. Clients without a trusted client
certificate cannot load the documentation.

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

Swagger UI itself is served over the same HTTPS listener. Operators typically:

1. open ``https://<host>:4202/doc/``
2. trust the server certificate if it is private or self-signed
3. authenticate through ``POST /api/v1/authenticate``
4. use the returned bearer token in Swagger UI for protected operations

If the server uses a self-signed certificate, browsers and CLI clients must
explicitly trust that certificate. Setting ``api.tls.allow-insecure: true``
only allows Sysinspect to start with that certificate posture. It does not
disable client-side certificate validation.

If the Web API is configured for mutual TLS, missing or untrusted client
certificates cause the TLS handshake to fail before Swagger UI or the OpenAPI
document can be served.

Production Recommendations
--------------------------

- keep ``api.tls.enabled: true``
- keep ``api.doc: true`` only when operators need interactive documentation
- prefer a CA-signed or otherwise centrally trusted certificate
- use ``api.tls.ca-file`` when operator client certificates are required
- keep ``api.devmode: false`` outside local development

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
