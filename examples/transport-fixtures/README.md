# Secure transport fixtures

These files are minimal example fixtures for the current transport and Web API
shapes.

They are illustrative fixtures, not captured live traffic dumps.

Files:

- `bootstrap_hello.json`: plaintext secure bootstrap opening
- `bootstrap_ack.json`: plaintext secure bootstrap acknowledgement
- `secure_data_frame.json`: encrypted steady-state frame envelope
- `webapi_auth_request.json`: HTTPS JSON authentication request body
- `webapi_query_request.json`: HTTPS JSON query request body

Use them for:

- documentation examples
- fixture-driven tests
- quick contract reviews during API or protocol changes
