# buildfarm

`buildfarm` is a terminal program for running the same build on more than one machine at the same time. A typical use case is a project that builds locally, but also needs to be built on a FreeBSD or Linux VM. Instead of opening several terminals, syncing files by hand, and trying to remember what was run where, `buildfarm` does the synchronization, starts the builds, and shows the output in one screen.

The important thing to understand is that `buildfarm` is not supposed to know anything about the private structure of a particular project. It should know how to send a project tree to a target machine, how to run one build entry there, and, if requested, how to copy back the final results. The project itself remains responsible for deciding what those final results are.

## The basic idea

Every `buildfarm` run has three parts. First, it reads a small text file that describes the build targets. Second, it starts the selected build entry on every target. Third, if result mirroring is enabled, it copies back only the files that the project explicitly listed as final deliverables.

In practice, the command usually looks like this:

```bash
export BUILDFARM_CONFIG=buildfarm.conf
buildfarm run devel
```

If the finished artifacts should also be copied back to the local machine, the command becomes:

```bash
export BUILDFARM_CONFIG=buildfarm.conf
buildfarm run devel --mirror-results
```

That is the whole workflow. The rest of the documentation explains what the config file looks like and what the producer project must provide.

## The target config file

The target config file is plain text. Each line describes one build target. The format is:

```text
<uname -o> <uname -m> [user@]host:/destination
```

Here is a small example:

```text
local
FreeBSD amd64 builder@freebsd-vm:work/example-buildfarm
GNU/Linux x86_64 builder@linux-vm:work/example-buildfarm
```

The special word `local` means that the current machine should also participate in the run. The other lines describe remote machines. For example, the FreeBSD line says that the project should be synchronized to `builder@freebsd-vm:work/example-buildfarm` and built there.

This means that one `buildfarm` run can cover the local machine and one or more remote systems with the same build entry.

## What the project must provide

The producer project must provide two things. First, the selected build entry must actually exist and work on the target machine. Second, if result mirroring is enabled, the project must write a manifest file that lists the files that should be copied back.

The standard manifest path is:

```text
build/.buildfarm/<entry>.paths
```

For a build entry named `devel`, the manifest path would therefore be:

```text
build/.buildfarm/devel.paths
```

The manifest format is intentionally simple. It is just a line-based list of relative paths:

```text
build/stage/hello
build/stage/hello-helper
build/dist/example.wasm
```

Each line names one file or directory that should be copied back after a successful build. Blank lines are ignored. Lines beginning with `#` are treated as comments. Paths are interpreted relative to the project root.

This manifest is the boundary between `buildfarm` and the producer project. The project decides what counts as a final deliverable. `buildfarm` simply copies what the manifest names.

## What happens during a run

For a remote target, `buildfarm` first synchronizes the current project tree to the destination directory with `rsync`. After that, it starts the selected build entry on the remote machine. On Linux-like systems it uses `make`. On FreeBSD it uses `gmake`. For the local target it simply runs the local build command without SSH.

If mirroring is enabled and the build succeeds, `buildfarm` reads the manifest file for that entry and copies back only the listed paths. The copied files land under:

```text
target/buildfarm/<OS-LABEL>/...
```

So, for example, a successful run might produce:

```text
target/buildfarm/freebsd_14.2/build/stage/hello
target/buildfarm/linux_6_glibc_2.39/build/stage/hello
```

Each target is handled independently. If one machine finishes before another, it can already start mirroring while the other machine is still compiling.

## A minimal Makefile example

The example below shows the smallest useful producer integration. It builds one executable into `build/stage` and then writes a manifest that lists that executable as the result worth copying back.

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

With that Makefile in place, a matching `buildfarm.conf` could look like this:

```text
local
FreeBSD amd64 builder@freebsd-vm:work/hello-buildfarm
```

Now a full run becomes:

```bash
export BUILDFARM_CONFIG=buildfarm.conf
buildfarm run devel --mirror-results
```

In this example, the local machine runs `make devel`, the FreeBSD target runs `gmake devel`, and the file listed in `build/.buildfarm/devel.paths` is copied back into the local `target/buildfarm/...` tree.

## Common commands

The most common form is:

```bash
buildfarm run devel
```

To enable result mirroring:

```bash
buildfarm run devel --mirror-results
```

To override the default local destination for mirrored results:

```bash
buildfarm run devel --mirror-results --mirror-root /tmp/buildfarm-out
```

To use an explicit config file instead of `BUILDFARM_CONFIG`:

```bash
buildfarm --config buildfarm.conf run devel
```

## Runtime logs

While the TUI is running, `buildfarm` keeps temporary logs in `.buildfarm/`. These logs are only runtime scratch data. They are not part of the producer contract and they are not where the manifest lives.

At the finish popup, `Ctrl-C` quits and deletes those logs. Pressing `p` quits and preserves them so they can still be inspected afterwards. Any other key dismisses the popup and leaves the TUI open.
