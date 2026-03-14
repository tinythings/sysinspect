% SYSINSPECT(8) Version 0.1

NAME
====

**sysinspect** — a tool for inspecting any kind of system, based on
  its architecture model

SYNOPSIS
========

| **sysinspect** \[**OPTIONS**]...
| **sysinspect** *model* \[*query*] \[**--traits** *traits-query*] \[**--context** *k:v,...*]
| **sysinspect** **--model** *path* \[**--entities** *list* | **--labels** *list*] \[**--state** *name*]
| **sysinspect** **traits** \[**--set** *k:v,...* | **--unset** *k,...* | **--reset**] \[**--id** *id* | **--query** *glob* | *glob*] \[**--traits** *query*]
| **sysinspect** **profile** \[**--new** | **--delete** | **--list** | **-A** | **-R** | **--tag** | **--untag**] ...
| **sysinspect** **module** \[**-A** | **-R** | **-L** | **-i**] ...

DESCRIPTION
===========

The **sysinspect** tool is a testing framework to inspect any kind of
system for the following means:

- Root Cause Analysis
- Anomaly Detection

The command-line tool talks to the local or configured master instance,
submits model requests, manages the module repository, updates
master-managed static traits on minions, and manages deployment profiles.

RUNNING MODELS REMOTELY
=======================

The most common use of **sysinspect** is sending a model request to the
master.

Examples:

| **sysinspect** "my_model"
| **sysinspect** "my_model/my_entity"
| **sysinspect** "my_model/my_entity/my_state"
| **sysinspect** "my_model" "*"
| **sysinspect** "my_model" "web*"
| **sysinspect** "my_model" "db01,db02"
| **sysinspect** "my_model" "*" **--traits** "system.os.name:Ubuntu"
| **sysinspect** "my_model" "*" **--context** "foo:123,name:Fred"

The optional second positional argument targets minions by hostname glob
or comma-separated host list. The **--traits** option further narrows the
target set. The **--context** option passes comma-separated key/value
data into the model call.

RUNNING MODELS LOCALLY
======================

**sysinspect** can also execute a model locally without going through the
master.

Examples:

| **sysinspect** **--model** ./my_model
| **sysinspect** **--model** ./my_model **--entities** foo,bar
| **sysinspect** **--model** ./my_model **--labels** os-check
| **sysinspect** **--model** ./my_model **--state** online

The local selector options are:

- **--entities** limit execution to specific entities
- **--labels** limit execution to specific labels
- **--state** choose the state to process

CLUSTER COMMANDS
================

- **--sync** refreshes cluster artefacts and triggers minions to report
  fresh traits back to the master
- **--online** prints the current online-minion summary to standard output
- **--shutdown** asks the master to stop
- **--unregister** *id* unregisters a minion by System Id

TRAITS
======

The **traits** subcommand updates only the master-managed static trait
overlay stored on minions.

Examples:

| **sysinspect** **traits** **--set** "foo:bar"
| **sysinspect** **traits** **--set** "foo:bar,baz:qux" "web*"
| **sysinspect** **traits** **--set** "foo:bar" **--id** 30006546535e428aba0a0caa6712e225
| **sysinspect** **traits** **--unset** "foo,baz" "web*"
| **sysinspect** **traits** **--reset** **--id** 30006546535e428aba0a0caa6712e225

Supported selectors:

- **--id** target one minion by System Id
- **--query** or trailing positional query target minions by hostname glob
- **--traits** further narrow the target set by traits query

PROFILES
========

Deployment profiles define which modules and libraries a minion is
allowed to sync.

Examples:

| **sysinspect** **profile** **--new** **--name** Toto
| **sysinspect** **profile** **--delete** **--name** Toto
| **sysinspect** **profile** **--list**
| **sysinspect** **profile** **--list** **--name** 'T*'
| **sysinspect** **profile** **-A** **--name** Toto **--match** 'runtime.lua,net.*'
| **sysinspect** **profile** **-A** **--lib** **--name** Toto **--match** 'runtime/lua/*.lua'
| **sysinspect** **profile** **-R** **--name** Toto **--match** 'net.*'
| **sysinspect** **profile** **--tag** 'Toto,Foo' **--query** 'web*'
| **sysinspect** **profile** **--untag** 'Foo' **--traits** 'system.hostname.fqdn:db01.example.net'

Notes:

- **--name** is an exact profile name for **--new**, **--delete**,
  **-A**, and **-R**
- **--name** is a glob pattern for **--list**
- **--match** accepts comma-separated exact names or glob patterns
- **-l** or **--lib** switches selector operations and listing to
  library selectors
- **--tag** and **--untag** update the **minion.profile** static trait
- a profile file carries its own canonical **name** field; the filename
  is only storage

MODULE REPOSITORY
=================

The **module** subcommand manages the master's module repository.

Examples:

| **sysinspect** **module** **-A** **--name** runtime.lua **--path** ./target/debug/runtime/lua
| **sysinspect** **module** **-A** **--path** ./lib **-l**
| **sysinspect** **module** **-L**
| **sysinspect** **module** **-Ll**
| **sysinspect** **module** **-R** **--name** runtime.lua
| **sysinspect** **module** **-R** **--name** runtime/lua/reader.lua **-l**
| **sysinspect** **module** **-i** **--name** runtime.lua

UTILITY COMMANDS
================

Additional operator entrypoints:

| **sysinspect** **--ui**
| **sysinspect** **--list-handlers**

**--ui** starts the terminal user interface. **--list-handlers** prints
the registered event handler identifiers.

COMMON OPTIONS
==============

- **-c**, **--config** *path* use an alternative configuration file
- **-d**, **--debug** increase log verbosity; repeat for more verbosity
- **-h**, **--help** display help
- **-v**, **--version** display version

DETAILED DOCUMENTATION
======================

See Sysinspect documentation online: <https://sysinspect.readthedocs.io/en/latest/>

BUGS
====

See GitHub Issues: <https://github.com/tinythings/sysinspect/issues>

AUTHOR
======

Bo Maryniuk <bo@maryniuk.net>
