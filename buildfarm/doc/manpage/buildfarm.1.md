% buildfarm(1)
% Sysinspect Project
% April 2026

# NAME

buildfarm - run one build entry across local and remote machines in parallel

# SYNOPSIS

```text
buildfarm [OPTIONS] init
buildfarm [OPTIONS] run <entry> [--mirror-results] [--mirror-root <dir>]
```

# DESCRIPTION

`buildfarm` is a terminal UI tool for projects that need to run the same build on more than one machine. It is intended for situations where a project builds locally, but also has to be built on remote systems such as FreeBSD or Linux virtual machines.

During a run, `buildfarm` reads a target list, synchronizes the project tree to remote targets, starts the selected build entry on every target, and shows the output in one screen. If result mirroring is enabled, it copies back only the files listed by the producer project’s manifest.

The tool is intentionally separate from project-specific build logic. It should know how to run builds and copy results, but the producer project remains responsible for deciding what the final deliverables are.

# TARGET CONFIG

The target config file is plain text. Each line describes one target in the following form:

```text
<uname -o> <uname -m> [user@]host:/destination
```

Example:

```text
local
FreeBSD amd64 builder@freebsd-vm:work/example-buildfarm
GNU/Linux x86_64 builder@linux-vm:work/example-buildfarm
```

The special value `local` means that the current machine should also take part in the run.

# PRODUCER CONTRACT

The producer project must provide a working build entry and, when mirroring is enabled, a manifest that lists the files to copy back.

The standard manifest path is:

```text
build/.buildfarm/<entry>.paths
```

For example:

```text
build/.buildfarm/devel.paths
```

The manifest is a line-based list of relative paths:

```text
build/stage/mytool
build/stage/myhelper
build/dist/example.wasm
```

Blank lines are ignored. Lines beginning with `#` are treated as comments.

# COMMANDS

## init

Validate and load the configured targets.

This currently checks the target configuration, but is not yet a full standalone remote bootstrap command.

## run <entry>

Run one build entry across all configured targets.

Examples:

```bash
buildfarm run devel
buildfarm run release
buildfarm run devel --mirror-results
buildfarm run devel --mirror-results --mirror-root /tmp/buildfarm-out
```

# OPTIONS

## -c, --config <file>

Use the given config file instead of `BUILDFARM_CONFIG`.

## --mirror-results

After a successful build, read `build/.buildfarm/<entry>.paths` and copy back only the listed files.

Mirroring is disabled by default.

## --mirror-root <dir>

Override the default local destination for mirrored results:

```text
./target/buildfarm
```

This option requires `--mirror-results`.

## -d, --debug

Increase debug verbosity.

# HOW A RUN WORKS

For each configured target, `buildfarm` synchronizes the project tree if the target is remote, then starts the selected build entry on that machine. Linux-like targets use `make`. FreeBSD targets use `gmake`.

If mirroring is enabled and the build succeeded, `buildfarm` reads the manifest for the selected entry and copies back only the listed files. Mirrored results are stored under:

```text
target/buildfarm/<OS-LABEL>/...
```

Targets are handled independently. One target may already be mirroring results while another is still compiling.

# SIMPLE MAKEFILE EXAMPLE

```make
STAGE_DIR := build/stage
BUILDFARM_MANIFEST_DIR := build/.buildfarm

.PHONY: devel

devel:
	@mkdir -p $(STAGE_DIR)
	@cc -O0 -g src/main.c -o $(STAGE_DIR)/hello
	@mkdir -p $(BUILDFARM_MANIFEST_DIR)
	@printf '%s\n' \
		'build/stage/hello' \
		> $(BUILDFARM_MANIFEST_DIR)/devel.paths
```

This example produces one executable and writes a manifest that lists it as the result to copy back.

# ENVIRONMENT

## BUILDFARM_CONFIG

Path to the buildfarm config file.

## BUILDFARM_LOCAL_MAKE

Override the command used for the local target.

Default:

```text
make
```

# FILES

## Producer manifest

```text
build/.buildfarm/<entry>.paths
```

This file belongs to the producer project and is required when `--mirror-results` is used.

# NOTES

While the TUI is running, `buildfarm` stores temporary logs in `.buildfarm/`. These logs are runtime scratch data only.

At the finish popup, `Ctrl-C` quits and deletes the logs. Pressing `p` quits and preserves them. Any other key dismisses the popup and keeps the TUI open.

# EXIT STATUS

`0` on success.

Non-zero if any target build or mirror step fails, or if the config or CLI arguments are invalid.

# SEE ALSO

`make(1)`, `gmake(1)`, `ssh(1)`, `rsync(1)`
