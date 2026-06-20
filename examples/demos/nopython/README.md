# Python Without Host Python

This demo shows how SysInspect can execute Python-based runtime payloads on a
target that does not provide a system `python3` binary.

The execution path is intentionally split into two layers:

- native SysInspect modules collect host facts and drive the decision logic
- the embedded `runtime.py3` runtime executes Python only when the model DSL
  determines that the target matches the intended profile

This keeps the Python payload focused on proof of execution rather than policy
or branching logic.

## Purpose

The demo is designed for compact or appliance-style targets where host Python
should not be treated as a deployment prerequisite.

It demonstrates that SysInspect can:

- identify the target environment using native modules
- evaluate model constraints before runtime dispatch
- execute Python code through the embedded runtime when the target matches
- deliver Python helper libraries as repository payloads rather than operating
  system packages

The model is fully local at execution time and does not depend on outbound
network access from the minion.

## Model Behavior

The model evaluates four host-side facts:

- target is Alpine
- target is Fedora
- host `python3` is absent
- host `python3` is present

Based on those facts, the model selects one applicable branch:

- Alpine without host Python:
  - run `py3.nopython`
- Fedora:
  - return a verification message from YAML
- non-Fedora hosts with host Python present:
  - return a verification message from YAML
- non-Alpine, non-Fedora hosts without host Python:
  - return a generic verification message from YAML

The Python payload itself returns runtime proof only, including interpreter
identity, version, selected runtime values, and helper-import evidence.

## Repository Contents

Files in this directory:

- `model.cfg`
- `lib/runtime/python3/nopython.py`
- `lib/runtime/python3/nopyproof.py`
- `lib/runtime/python3/site-packages/nopykit/__init__.py`

## Scope And Entities

Install the model under the `nopython` scope.

This demo also declares an explicit public interface:

```yaml
interface:
  entities:
    - all
  actions:
    - python-proof
```

The `interface` section is optional.

- if `interface` is present, only listed entity and action entrypoints are
  considered part of the model's public surface
- if `interface` is absent, the current legacy behavior remains in effect and
  all inferred entrypoints are public

The interface section does not alter internal execution semantics. It describes
which entrypoints are intended to be exposed to callers.

Entities exposed by this model:

- `all`
- `python-proof`
- `verify-fedora`
- `verify-python`
- `verify-other`

## Prerequisites

The master must provide:

- the `runtime.py3` dispatcher module
- the `nopython` model scope
- the runtime Python payload tree from this directory

The minion does not need a system `python3` package.

## Install The Model

Copy `model.cfg` into the master's models root:

```text
$MASTER/data/models/nopython/model.cfg
```

Export the scope from the master configuration:

```yaml
config:
  master:
    fileserver.models:
      - nopython
```

## Install The Embedded Python Runtime

Build and register `runtime.py3`:

```bash
make all-devel
sysinspect module -A --path ./target/debug/runtime/py3-runtime --name runtime.py3 --descr "Python 3 runtime"
```

## Install The Python Runtime Payload Tree

This demo ships its payloads under `lib/` so the runtime directory structure is
preserved during publication.

From `examples/demos/nopython`, publish the `lib` tree itself:

```bash
sysinspect module -A --path ./lib -l
```

Then sync the cluster:

```bash
sysinspect --sync
```

Important:

- select or publish `examples/demos/nopython/lib`
- do not publish the parent directory `examples/demos/nopython`

The runtime expects the published tree to land under:

- `lib/runtime/python3/`
- `lib/runtime/python3/site-packages/`

## Python Runtime Layout Rules

The embedded Python runtime expects the following repository layout:

- executable Python modules:
  - `lib/runtime/python3/`
- helper libraries:
  - `lib/runtime/python3/site-packages/`

For this demo, that means:

- `lib/runtime/python3/nopython.py`
- `lib/runtime/python3/nopyproof.py`
- `lib/runtime/python3/site-packages/nopykit/__init__.py`

Each runtime Python module should export:

- `run(req)` as the entrypoint
- optional documentation as either:
  - `doc = {...}`
  - `def doc(): return {...}`

The documentation object must be the payload itself, not wrapped as
`{"doc": ...}`.

## Verify Installation

Inspect the published runtime payloads:

```bash
sysinspect module -Ll
```

Expected library entries include:

- `runtime/python3/nopython.py`
- `runtime/python3/nopyproof.py`
- `runtime/python3/site-packages/nopykit/__init__.py`

## Run The Demo

Run the full model:

```bash
sysinspect "nopython/all" '*'
```

Or run one branch-oriented entity directly:

```bash
sysinspect "nopython/python-proof" '*'
sysinspect "nopython/verify-fedora" '*'
sysinspect "nopython/verify-python" '*'
sysinspect "nopython/verify-other" '*'
```

## Expected Outcomes

When the Python proof branch is selected, the action should return structured
runtime evidence such as:

- Python runtime implementation name
- Python runtime version
- Python platform and byte order
- selected `sys` values
- helper module import evidence

The branch decision itself is made by the model DSL, not by the Python payload.

Typical outcomes:

- Alpine without host Python:
  - `python-proof` is applicable
- Fedora:
  - `verify-fedora` is applicable
- non-Fedora with host Python present:
  - `verify-python` is applicable
- non-Alpine, non-Fedora without host Python:
  - `verify-other` is applicable

Non-selected branches are reported as `Not Applicable`, not as execution
failures.
