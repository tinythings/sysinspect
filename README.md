# Sysinspect

![insanity_workflow](https://github.com/tinythings/sysinspect/actions/workflows/insanity_check.yml/badge.svg)
![build_workflow](https://github.com/tinythings/sysinspect/actions/workflows/generic_workflow.yml/badge.svg)
![build_workflow](https://github.com/tinythings/sysinspect/actions/workflows/all_the_tests.yml/badge.svg)
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
This workflow works very similar to Ansible style.

## Documentation

Complete documentation can be found here:
[https://sysinspect.readthedocs.io/en/latest/](https://sysinspect.readthedocs.io/en/latest/)

## Building & Requirements

SysInspect is currently built on Ubuntu 22.04 or 24.04.

### Dependencies

Your system should have the following packages (Ubuntu/Debian):

- pkg-config
- libssl-dev
- libffi-dev

Or equivalent names in your Linux distribution.

### Building

To build Sysinspect, **do not** use `cargo` directly, but use GNU
Make. First, you need to setup your environment:

  make setup <ENTER>

Once this is done, you can make a release build:

  make <ENTER>

Binary will be found in `/target/release` and modules in
`/target/release/sys` (currently only `sys.` namespace implemented).

For static compilation you can use `musl-x86_64-dev` or `musl-x86_64`
targets. For cross-compilation on ARM you can use `musl-aarch64-dev`
or `musl-aarch64`, but the cross compiler for linking
(`aarch64-linux-gnu-gcc`) must be installed separately.
