# sysclient

Small handwritten example client for the SysInspect Web API.

The legacy generated OpenAPI client was removed together with the old Web API
application-layer crypto scheme. This crate now talks to the API directly over
plain JSON requests.

See a practical example in `src/main.rs`.
