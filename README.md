# Sysinspect

![insanity_workflow](https://github.com/tinythings/sysinspect/actions/workflows/insanity_check.yml/badge.svg)
![build_workflow](https://github.com/tinythings/sysinspect/actions/workflows/generic_workflow.yml/badge.svg)
![build_workflow](https://github.com/tinythings/sysinspect/actions/workflows/all_the_tests.yml/badge.svg)
[![Docs](https://readthedocs.org/projects/sysinspect/badge/?version=latest)](https://sysinspect.readthedocs.io/en/latest/)
[![Release](https://img.shields.io/github/v/release/tinythings/sysinspect)](https://github.com/tinythings/sysinspect/releases)
[![Rust](https://img.shields.io/badge/Rust-1.94.1%2B-orange.svg)](https://github.com/tinythings/sysinspect/blob/master/rust-toolchain.toml)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)



Sysinspect is an engine of Anomaly Detection and Root Cause Analysis.
This engine is indented to perform anomaly detection and root cause
analysis on any system. It is using Model Description as a source of
knowledge and a collection of modules with telemetry data in order to
perform various testing scenarios. 

## Use Case

Primarily this is an experimental work for Anomaly Detection (AD) and
Root Cause Analysis (RCA).

## Bonus Use Case

Configuration management. As Sysinspect can get the information, it
also can set the information. It is a matter of modules.
This workflow follows a model-driven configuration approach.

## Documentation

Complete documentation can be found here:
[https://sysinspect.readthedocs.io/en/latest/](https://sysinspect.readthedocs.io/en/latest/)

## Building & Requirements

SysInspect is built with GNU Make.

Do not build this project by calling `cargo` directly at the workspace root.

### Linux

On modern Linux distributions, including current Ubuntu and Debian releases, bootstrap the toolchain and build dependencies with:

```bash
make setup
```

Then build:

```bash
make
```

For a development build:

```bash
make dev
```

For modules only:

```bash
make modules
make modules-dev
```

### FreeBSD

On FreeBSD, plain `make` is BSD make, while this project uses GNU make syntax.

Use this flow:

```bash
make setup
```

That bootstrap step uses [`BSDmakefile`](BSDmakefile) to install `gmake` and the required Rust/build packages, then hands off to `gmake setup`.

After that, use `gmake` for all real builds:

```bash
gmake
gmake dev
gmake modules
gmake modules-dev
gmake test
gmake smoke-test
```

### Buildfarm

If `BUILDFARM_CONFIG` is exported, buildfarm-aware entries use the standalone `buildfarm` TUI.

Bootstrap once:

```bash
make setup
```

That prepares the normal toolchain and prebuilds:

```text
target/buildfarm/buildfarm
```

Manual rebuild of the controller:

```bash
make buildfarm
```

Prepare remote destinations:

```bash
export BUILDFARM_CONFIG=buildfarm.conf
make buildfarm-init
```

Then run any buildfarm-aware entry. Each run still delta-rsyncs local changes to remotes before building:

```bash
make dev
make all-dev
make modules-dev
make release
make all
make modules
```

Result mirroring is off by default.

To mirror staged results back from all targets into `target/buildfarm/...`, use:

```bash
BUILDFARM_MIRROR_RESULTS=1 make modules-dist-dev
```

To override the local mirror root:

```bash
BUILDFARM_MIRROR_RESULTS=1 BUILDFARM_MIRROR_ROOT=/tmp/buildfarm-out make modules-dist-dev
```

### Build Output

Binaries are produced by Cargo under `target/...`.

Packaged/staged artifacts are copied into:

```text
build/stage/
```

Module distribution payloads are staged into:

```text
build/modules-dist/
```

### Static Linux Builds

For static Linux builds, use:

```bash
make musl-x86_64
make musl-x86_64-dev
make musl-aarch64
make musl-aarch64-dev
```

Cross-compiling for AArch64 Linux still requires the matching cross linker to be installed on the host.
