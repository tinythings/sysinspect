# Sysinspect

![insanity_workflow](https://github.com/tinythings/sysinspect/actions/workflows/insanity_check.yml/badge.svg)
![build_workflow](https://github.com/tinythings/sysinspect/actions/workflows/generic_workflow.yml/badge.svg)
![build_workflow](https://github.com/tinythings/sysinspect/actions/workflows/all_the_tests.yml/badge.svg)
[![Docs](https://readthedocs.org/projects/sysinspect/badge/?version=latest)](https://sysinspect.readthedocs.io/en/latest/)
[![Release](https://img.shields.io/github/v/release/tinythings/sysinspect)](https://github.com/tinythings/sysinspect/releases)
[![Rust](https://img.shields.io/badge/Rust-1.95.0%2B-orange.svg)](https://github.com/tinythings/sysinspect/blob/master/rust-toolchain.toml)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Sysinspect is an experimental system inspection and analysis framework. Its central idea is that a
machine can be described through models, observed through telemetry, and then reasoned about in a
structured way. In practical terms, the project is aimed at anomaly detection and root cause
analysis, but the same machinery can also be used for model-driven configuration work when a module
is able not only to observe state, but also to change it.

The codebase is therefore a mixture of core libraries, service binaries, telemetry components, and
execution modules. If you are reading this project for the first time, it is best to think of
Sysinspect not as one single binary, but as a toolkit for collecting state, evaluating it, and
acting on it.

Complete user and developer documentation lives at:
[https://sysinspect.readthedocs.io/en/latest/](https://sysinspect.readthedocs.io/en/latest/)

## Building the project

Sysinspect is built through GNU Make. Although the project is written in Rust, the intended entry
point for normal development is not `cargo` at the workspace root. The Makefiles take care of the
build layout, staging rules, module packaging, and a number of platform-specific details. In other
words, if you invoke `cargo` directly at the top of the workspace, you are stepping around the part
of the build system that is actually responsible for assembling the project.

For that reason, the normal rule is simple: use `make` on Linux and use `gmake` on FreeBSD after bootstrapping the host.

### Linux

On Linux, begin by installing the required toolchain and system dependencies:

```bash
make setup
```

After that, the default build is simply:

```bash
make
```

This produces the regular release-oriented core build. If you want a development build with debug
information, use:

```bash
make dev
```

If you are working only on modules, the most relevant entries are:

```bash
make modules
make modules-dev
```

### FreeBSD

FreeBSD needs one extra sentence of explanation. On FreeBSD, plain `make` is BSD make, but this
project uses GNU Make syntax. The repository therefore includes a
[`BSDmakefile`](BSDmakefile) whose only job is to bootstrap the machine and then hand control over
to `gmake`.

The recommended first step on FreeBSD is:

```bash
make setup
```

That step installs `gmake` and the required build packages, then runs the normal GNU Make setup
flow. After that, use `gmake` for actual work:

```bash
gmake
gmake dev
gmake modules
gmake modules-dev
gmake test
gmake smoke-test
```

This may look slightly old-fashioned, but it keeps the FreeBSD path explicit and predictable.

## What the build produces

Rust’s normal compilation output still goes under `target/...`, because that is Cargo’s world.
Sysinspect’s staged and packaged outputs live elsewhere, because the project needs a stable place
for assembled artifacts that are ready to be used by the rest of the system.

The most important output directories are these:

```text
build/stage/
build/modules-dist/
```

`build/stage/` contains staged artifacts that the project treats as assembled output rather than
raw compiler products. `build/modules-dist/` contains distribution payloads for modules. If you are
tracing “what was actually built for use”, those directories are usually more informative than
`target/` alone.

## Using `xrun` Buildfarm

Sysinspect can also be built across several machines (OS/Platforms) at once with `xrun`, which is now a separate
standalone project. This is useful when the same build needs to be run locally and on one or more
remote systems, for example when a Linux workstation also has to produce a FreeBSD build.

To find more about `xrun`, visit the GitHub page here: [https://github.com/tinythings/xrun](https://github.com/tinythings/xrun)

The Makefiles are aware of `xrun` when `XRUN_CONFIG` is exported. In that mode, the normal build
entries such as `make dev` or `make modules` are still the commands you type, but they are
executed through the `xrun` runner instead of only on the local machine.

The normal bootstrap step is still:

```bash
make setup
```

To check that your xrun configuration is valid, use:

```bash
export XRUN_CONFIG=xrun.conf
make xrun-init
```

Once that is in place, the familiar build entries can run through the xrun matrix:

```bash
make dev
make all-dev
make modules-dev
make release
make all
make modules
```

Result mirroring is disabled by default. When it is enabled, the final deliverables listed by the
producer manifest are copied back into `target/xrun/...`.

For example:

```bash
make modules-dist-dev XRUN_ARGS="--mirror-results"
```

If you want a different local destination for mirrored results:

```bash
make modules-dist-dev XRUN_ARGS="--mirror-results --mirror-root /tmp/xrun-out"
```

The contract between Sysinspect and `xrun` is intentionally simple. Sysinspect writes the manifest
of final outputs under `build/.xrun/`, and `xrun` copies back only what that manifest names.

## Static Linux builds

The repository also includes convenience entries for static Linux builds against musl. These are
mainly useful when you want self-contained Linux artifacts without depending on the host’s glibc
environment.

The available entries are:

```bash
make musl-x86_64
make musl-x86_64-dev
make musl-aarch64
make musl-aarch64-dev
```

Cross-compiling to AArch64 still requires the appropriate cross linker to be installed on the host
system. The Makefile does not invent that part for you.
