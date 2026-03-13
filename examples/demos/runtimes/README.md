# Runtime Demo Pack

This demo directory contains one model that exercises all three runtime
families through the public virtual namespaces:

- `lua.reader`
- `py3.reader`
- `wasm.hellodude`

Files in this demo:

- `model.cfg`

The model binds three entities:

- `all`
- `demo-lua`
- `demo-py3`
- `demo-wasm`

and calls the following runtime modules:

- `demo-lua` -> `lua.reader`
- `demo-py3` -> `py3.reader`
- `demo-wasm` -> `wasm.hellodude`

What the model actually asks those modules to do:

- `lua.reader` reads `VERSION` from `/etc/os-release`
- `py3.reader` reads `VERSION` from `/etc/os-release`
- `wasm.hellodude` reads the `VERSION` key from `/etc/os-release`

## Master

1. Copy `model.cfg` to the master's models root into subdirectory `runtimes`,
   so you end up with:

   `$MASTER/data/models/runtimes/model.cfg`

2. Edit master configuration so this model scope is exported:

   ```yaml
   config:
     master:
       fileserver.models:
         - runtimes
   ```

3. Build and register the runtime dispatcher modules.

   If you build from the repository root with:

   ```bash
   make all-devel
   ```

   then the dispatcher binaries are placed under:

   - `target/debug/runtime/lua-runtime`
   - `target/debug/runtime/py3-runtime`
   - `target/debug/runtime/wasm-runtime`

   Register them on the master:

   ```bash
   sysinspect module -A --path ./target/debug/runtime/lua-runtime --name runtime.lua --descr "Lua runtime"
   sysinspect module -A --path ./target/debug/runtime/py3-runtime --name runtime.py3 --descr "Python 3 runtime"
   sysinspect module -A --path ./target/debug/runtime/wasm-runtime --name runtime.wasm --descr "Wasm runtime"
   ```

4. Publish the Lua runtime example payload tree:

   ```bash
   sysinspect module -A --path ./modules/runtime/lua-runtime/examples/lib -l
   ```

   This tree contains:

   - `lib/runtime/lua/reader.lua`
   - `lib/runtime/lua/hello.lua`
   - `lib/runtime/lua/caller.lua`

5. Publish the Python runtime example payload tree:

   ```bash
   sysinspect module -A --path ./modules/runtime/py3-runtime/examples/lib -l
   ```

   This tree contains:

   - `lib/runtime/python3/hello.py`
   - `lib/runtime/python3/reader.py`
   - `lib/runtime/python3/caller.py`
   - `lib/runtime/python3/site-packages/mathx/__init__.py`

6. Build and publish the Wasm payload used by this model.

   The model uses the Wasm guest under:

   - `modules/runtime/wasm-runtime/examples/hello/main.go`

   Build it using the example's own Makefile. That Makefile writes the runtime
   payload tree under:

   - `modules/runtime/wasm-runtime/examples/hello/build/lib`

   Example:

   ```bash
   make -C ./modules/runtime/wasm-runtime/examples/hello WASM_COMPILER=go release
   sysinspect module -A --path ./modules/runtime/wasm-runtime/examples/hello/build/lib -l
   ```

   If you use TinyGo instead, replace `WASM_COMPILER=go` with
   `WASM_COMPILER=tinygo`.

7. Sync the cluster:

   ```bash
   sysinspect --sync
   ```

8. Verify the model scope is present and the runtime payloads were published:

   ```bash
   sysinspect module -L
   sysinspect module -Ll
   ```

9. Verify the model was exported by the master.

   Sysinspect model queries use the path form:

   - `/<model>/[entity]/[state]`

   Since this demo installs the model under the `runtimes` scope and defines
   entities `all`, `demo-lua`, `demo-py3`, and `demo-wasm`, the concrete query paths are:

   - `runtimes/all`
   - `runtimes/demo-lua`
   - `runtimes/demo-py3`
   - `runtimes/demo-wasm`

## Minion

Nothing special is required beyond normal sync.

The minion must receive:

- the exported `runtimes` model scope
- the runtime dispatcher modules
- the runtime library payloads

## Optional development shortcut

From the repository root, the `Makefile` now has a development refresh target:

```bash
make modules-refresh-devel
```

That target rebuilds and refreshes:

- all module dispatcher binaries
- the Lua runtime example library tree
- the Python runtime example library tree
- the Wasm example payloads currently wired into the development refresh flow

This is a development helper. You still need to place `model.cfg` into the
master's models root and ensure `fileserver.models` exports `runtimes`.

## Run the demo

To run the whole model across all synced minions:

```bash
sysinspect "runtimes/all" '*'
```

To run one runtime-backed entity at a time:

```bash
sysinspect "runtimes/demo-lua" '*'
sysinspect "runtimes/demo-py3" '*'
sysinspect "runtimes/demo-wasm" '*'
```

The query strings above are derived directly from:

- the exported model scope: `runtimes`
- the entity names present in `model.cfg`, including the aggregate `all`

## What to expect

Once the model is installed and synced, these are the concrete values expected
from the shipped example code:

- `demo-lua` returns a payload with `version`
- `demo-py3` returns a payload with `version`
- `demo-wasm` returns a payload with:
  - `VERSION`
  - `output: "hello, dude"`

This README intentionally describes only files, paths, and build/publish flows
that exist in the repository right now.
