Deployment Profile Examples
===========================

This directory contains example deployment profile files for the master-side
`profiles.index` / `<name>.profile` mechanism.

Files:

- `tiny-lua.profile`
  - a narrow profile that allows only the Lua runtime and Lua-side libraries
- `runtime-full.profile`
  - a fuller runtime profile that allows Lua, Py3, and Wasm runtimes together

Profile file format:

```yaml
name: tiny-lua
modules:
  - runtime.lua
libraries:
  - lib/runtime/lua/*.lua
```

Notes:

- profile identity comes from `name`, not the filename
- selectors support exact names and glob patterns
- effective minion selection is driven by the `minion.profile` trait
