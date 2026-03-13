# Runtime Demo Pack

This demo gives us one working model that exercises all three runtime families
through the public virtual namespaces:

- `lua.reader`
- `py3.hello`
- `wasm.hellodude`

The model is in `model.cfg` in this directory.

## What this demo does

- `demo-lua` reads `VERSION` from `/etc/os-release` through the Lua runtime example.
- `demo-py3` runs the Python `hello.py` example and returns the calculated sum.
- `demo-wasm` runs the Wasm `hellodude` example and returns the selected `/etc/os-release` key.

Expected outcomes:

- `demo-lua` returns data containing `version`
- `demo-py3` returns data containing `sum`
- `demo-wasm` returns data containing `VERSION` and `output`

## Install the runtime dispatchers

Build the workspace first:

```bash
make all-devel
```

Register the runtime binaries on the SysMaster:

```bash
sysinspect module -A --path ./target/debug/runtime/lua-runtime --name runtime.lua --descr "Lua runtime"
sysinspect module -A --path ./target/debug/runtime/py3-runtime --name runtime.py3 --descr "Python 3 runtime"
sysinspect module -A --path ./target/debug/runtime/wasm-runtime --name runtime.wasm --descr "Wasm runtime"
```

Then sync:

```bash
sysinspect --sync
```

## Install the Lua example payload

The Lua runtime example payload already has the correct `lib/runtime/lua` layout.
From the repository root:

```bash
sysinspect module -A --path ./modules/runtime/lua-runtime/examples/lib -l
sysinspect --sync
```

This installs `reader.lua` and its siblings under the `runtime.lua` library tree.

## Install the Python example payload

The Python runtime example payload already has the correct `lib/runtime/python3` layout.
From the repository root:

```bash
sysinspect module -A --path ./modules/runtime/py3-runtime/examples/lib -l
sysinspect --sync
```

This installs `hello.py`, `reader.py`, `caller.py`, and the `site-packages` helper package.

## Build and install the Wasm example payload

The Wasm demo uses the `hellodude` guest from `modules/runtime/wasm-runtime/examples/hello`.
Build it there first:

```bash
make -C ./modules/runtime/wasm-runtime/examples/hello
```

That should produce `hellodude.wasm` in the example build output. Stage it into a
runtime library layout and publish it as a library payload:

```bash
mkdir -p /tmp/sysinspect-runtime-demo/lib/runtime/wasm
cp ./modules/runtime/wasm-runtime/examples/hello/build/hellodude.wasm /tmp/sysinspect-runtime-demo/lib/runtime/wasm/
sysinspect module -A --path /tmp/sysinspect-runtime-demo/lib -l
sysinspect --sync
```

## Run the demo

Target one entity at a time if you want to inspect the runtimes separately:

```bash
sysinspect examples/demos/runtimes/demo-lua '*'
sysinspect examples/demos/runtimes/demo-py3 '*'
sysinspect examples/demos/runtimes/demo-wasm '*'
```

Or run the whole model if that is how you normally drive demos in your setup.

## What success looks like

Typical successful output should resemble this:

- Lua:
  - `message` indicates success
  - `data.version` contains the host OS version string
  - `__sysinspect-module-logs` contains the Lua log line because `rt.logs` is enabled

- Py3:
  - `data.sum` is `12`

- Wasm:
  - `data.VERSION` contains the `/etc/os-release` `VERSION` value
  - `data.output` is `hello, dude`

## Notes

- `rt.*` names are runtime-internal and are not used directly in the model DSL here.
- The model uses the stable public namespaces only: `lua.*`, `py3.*`, and `wasm.*`.
- If the Wasm example build output path differs on your machine, adjust the `cp` command accordingly.
