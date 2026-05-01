.DEFAULT_GOAL := help

include Makefile.in

XRUN_BIN := xrun
XRUN_ARGS ?=

.PHONY: help release xrun xrun-init xrun-status set-local-builds set-remote-builds build dev all all-dev modules modules-dev modules-dist-dev modules-refresh-dev modules-refresh clean check fix setup smoke-test \
	musl-aarch64-dev musl-aarch64 musl-x86_64-dev musl-x86_64 \
	stats man test test-core test-modules test-sensors test-integration tar dev-tls \
	_dev _all_dev _all _build _modules_dev _modules _modules_dist_dev _test _test_core _test_modules _test_sensors _test_integration

help:
	@printf '\n$$ make [help]\n\n'
	@printf '\033[1;92m%s\033[0m\n' "Development"
	@printf '    \033[1;93m%-20s\033[0m %s\n' "help" "Show this help and what each entry does."
	@printf '    %-20s %s\n' "dev" "Compile core binaries in development mode with debug data."
	@printf '    %-20s %s\n' "all-dev" "Compile core plus modules in development mode."
	@printf '    %-20s %s\n' "modules-dev" "Compile modules only in development mode."
	@printf '    %-20s %s\n' "modules-dist-dev" "Build release modules and stage distribution payloads."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "modules-refresh-dev" "Debug variant of Linux musl module refresh."
	@printf '\n\033[1;92m%s\033[0m\n' "Release"
	@printf '    %-20s %s\n' "release" "Compile core binaries in release mode."
	@printf '    %-20s %s\n' "all" "Compile core plus modules in release mode."
	@printf '    %-20s %s\n' "modules" "Compile modules only in release mode."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "modules-refresh" "Rebuild Linux musl module repo and refresh current minion slot."
	@printf '\n\033[1;92m%s\033[0m\n' "Utils"
	@printf '    \033[1;93m%-20s\033[0m %s\n' "setup" "Install toolchain dependencies and Rust targets for this host."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "xrun-status" "Show whether xrun mode is active or local-only."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "set-local-builds" "Disable xrun; all builds run locally."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "set-remote-builds" "Enable xrun; builds run across the target matrix."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "xrun-init" "Validate xrun config and initialise targets."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "xrun" "Check that xrun binary and config are available."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "smoke-test" "Run platform smoke tests."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "check" "Run clippy in deny-warnings mode."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "fix" "Run clippy --fix on the workspace."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "clean" "Remove Cargo build output."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "stats" "Show code statistics via tokei."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "dev-tls" "Generate local development TLS material."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "tar" "Create a vendored source tarball."
	@printf '    \033[1;93m%-20s\033[0m %s\n' 'XRUN_ARGS="..."' "Pass extra CLI flags to xrun, e.g. --mirror-results or --mirror-root /tmp/out."
	@printf '\n\033[1;92m%s\033[0m\n' "Testing"
	@printf '    %-20s %s\n' "test" "Run the full nextest suite for this platform."
	@printf '    %-20s %s\n' "test-core" "Run core crate unit/bin tests only."
	@printf '    %-20s %s\n' "test-modules" "Run module tests only."
	@printf '    %-20s %s\n' "test-sensors" "Run sensor crate tests only."
	@printf '    %-20s %s\n' "test-integration" "Run integration tests only."
	@printf '\n\033[1;92m%s\033[0m\n' "Cross Builds"
	@printf '    \033[1;93m%-20s\033[0m %s\n' "musl-x86_64" "Build static x86_64 Linux release artifacts."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "musl-x86_64-dev" "Build static x86_64 Linux debug artifacts."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "musl-aarch64" "Build static AArch64 Linux release artifacts."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "musl-aarch64-dev" "Build static AArch64 Linux debug artifacts."
	@printf '\n\033[1;92m%s\033[0m\n' "Documentation"
	@printf '    \033[1;93m%-20s\033[0m %s\n' "man" "Build the sysinspect manpage from Markdown."
	@printf '\n\033[1;96m%s\033[0m\n' "xrun"
	@printf '    %s\n' "Use 'make xrun-status' to check current mode."
	@printf '    %s\n' "If xrun is active, build and test targets delegate to the target matrix."
	@printf '    %s\n' "If xrun is inactive, builds run locally as usual."
	@printf '\n'


xrun-status:
	sh scripts/xrun-status.sh

set-local-builds:
	sh scripts/xrun-set-local.sh

set-remote-builds:
	sh scripts/xrun-set-remote.sh

release: build

xrun-init: setup
	@command -v $(XRUN_BIN) >/dev/null 2>&1 || { echo "Missing $(XRUN_BIN). Install it first." >&2; exit 1; }
	@if [ ! -f xrun.conf ]; then echo "No xrun.conf found in this project. Create one first." >&2; exit 1; fi
	@XRUN_CONFIG=xrun.conf XRUN_LOCAL_MAKE='$(MAKE)' $(XRUN_BIN) init

xrun: setup
	@command -v $(XRUN_BIN) >/dev/null 2>&1 || { echo "Missing $(XRUN_BIN). Install it first." >&2; exit 1; }
	sh scripts/xrun-status.sh

setup:
	$(call deps)
	$(call setup_targets)

clean:
	cargo clean

check:
	cargo clippy --no-deps --workspace $(PLATFORM_WORKSPACE_EXCLUDES) -- -Dwarnings -Aunused-variables -Adead-code

fix:
	cargo clippy --fix --allow-dirty --allow-staged --workspace $(PLATFORM_WORKSPACE_EXCLUDES)

smoke-test:
	sh smoke-tests/run.sh

musl-aarch64-dev:
	$(call check_present,aarch64-linux-musl-gcc)
	cargo build -v --workspace $(MUSL_WORKSPACE_EXCLUDES) --target aarch64-unknown-linux-musl
	$(call stage_profile_modules,debug,aarch64-unknown-linux-musl)
	$(call stage_profile_minion,debug,aarch64-unknown-linux-musl)

musl-aarch64:
	$(call check_present,aarch64-linux-musl-gcc)
	cargo build --release --workspace $(MUSL_WORKSPACE_EXCLUDES) --target aarch64-unknown-linux-musl
	$(call stage_profile_modules,release,aarch64-unknown-linux-musl)
	$(call stage_profile_minion,release,aarch64-unknown-linux-musl)

musl-x86_64-dev:
	$(call check_present,x86_64-linux-musl-gcc)
	cargo build -v --workspace $(MUSL_WORKSPACE_EXCLUDES) --target x86_64-unknown-linux-musl
	$(call stage_profile_modules,debug,x86_64-unknown-linux-musl)
	$(call stage_profile_minion,debug,x86_64-unknown-linux-musl)

musl-x86_64:
	$(call check_present,x86_64-linux-musl-gcc)
	cargo build --release --workspace $(MUSL_WORKSPACE_EXCLUDES) --target x86_64-unknown-linux-musl
	$(call stage_profile_modules,release,x86_64-unknown-linux-musl)
	$(call stage_profile_minion,release,x86_64-unknown-linux-musl)

all-dev:
	@scripts/maybe-xrun.sh all-dev || $(MAKE) _all_dev

_all_dev:
	cargo build -v --workspace $(PLATFORM_WORKSPACE_EXCLUDES)
	$(call stage_profile_modules,debug,)
	$(call stage_profile_minion,debug,)
	$(call write_xrun_manifest,all-dev,)

all:
	@scripts/maybe-xrun.sh all || $(MAKE) _all

_all:
	cargo build --release --workspace $(PLATFORM_WORKSPACE_EXCLUDES)
	$(call stage_profile_modules,release,)
	$(call stage_profile_minion,release,)
	$(call write_xrun_manifest,all,)

dev:
	@scripts/maybe-xrun.sh dev || $(MAKE) _dev

_dev:
	cargo build -v --workspace $(CORE_EXCLUDES)
	$(call stage_profile_modules,debug,)
	$(call stage_profile_minion,debug,)
	$(call write_xrun_manifest,dev,)

build:
	@scripts/maybe-xrun.sh release || $(MAKE) _build

_build:
	cargo build --release --workspace $(CORE_EXCLUDES)
	$(call stage_profile_modules,release,)
	$(call stage_profile_minion,release,)
	$(call write_xrun_manifest,release,)

modules-dev:
	@scripts/maybe-xrun.sh modules-dev || $(MAKE) _modules_dev

_modules_dev:
	@CARGO_BUILD_JOBS=$(MODULE_BUILD_JOBS) cargo build -v $(foreach pkg,$(MODULE_PACKAGE_SPECS),-p $(pkg))
	$(call stage_profile_modules,debug,)
	$(call write_xrun_manifest,modules-dev,)

modules:
	@scripts/maybe-xrun.sh modules || $(MAKE) _modules

_modules:
	@CARGO_BUILD_JOBS=$(MODULE_BUILD_JOBS) cargo build --release $(foreach pkg,$(MODULE_PACKAGE_SPECS),-p $(pkg))
	$(call stage_profile_modules,release,)
	$(call write_xrun_manifest,modules,)

modules-dist-dev:
	@scripts/maybe-xrun.sh modules-dist-dev || $(MAKE) _modules_dist_dev

_modules_dist_dev:
	@CARGO_BUILD_JOBS=$(MODULE_BUILD_JOBS) cargo build --release $(foreach pkg,$(MODULE_PACKAGE_SPECS),-p $(pkg))
	$(call stage_profile_modules,release,)
	$(call stage_modules_dist)
	$(call write_xrun_manifest,modules-dist-dev,with-dist)

test: setup
	@scripts/maybe-xrun.sh test || $(MAKE) _test

_test:
	@CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run --no-fail-fast --workspace $(PLATFORM_WORKSPACE_EXCLUDES) --test-threads $(TEST_RUN_THREADS)

test-core: setup
	@scripts/maybe-xrun.sh test-core || $(MAKE) _test_core

_test_core:
	@CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run --no-fail-fast $(foreach pkg,$(CORE_PACKAGE_SPECS),-p $(pkg)) --lib --bins --test-threads $(TEST_RUN_THREADS)

test-modules: setup
	@scripts/maybe-xrun.sh test-modules || $(MAKE) _test_modules

_test_modules:
	@CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run --no-fail-fast $(foreach pkg,$(MODULE_PACKAGE_SPECS),-p $(pkg)) --bins --test-threads $(TEST_RUN_THREADS)

test-sensors: setup
	@scripts/maybe-xrun.sh test-sensors || $(MAKE) _test_sensors

_test_sensors:
	@CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run --no-fail-fast $(foreach pkg,$(SENSOR_PACKAGE_SPECS),-p $(pkg)) --lib --bins --test-threads $(TEST_RUN_THREADS)

test-integration: setup
	@scripts/maybe-xrun.sh test-integration || $(MAKE) _test_integration

_test_integration:
	@CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run --no-fail-fast $(INTEGRATION_TEST_TARGETS) --test-threads $(TEST_RUN_THREADS)

modules-refresh-dev:
	$(call tgt,wasm32-wasip1)
	@if [ -z "$(CURRENT_MUSL_TARGET)" ] || [ -z "$(CURRENT_MUSL_CC)" ]; then \
		echo "modules-refresh-dev currently supports only configured Linux musl hosts; current host is $(UNAME_S)/$(UNAME_M)." >&2; \
		exit 1; \
	fi
	$(call tgt,$(CURRENT_MUSL_TARGET))
	$(call check_present,$(CURRENT_MUSL_CC))
	cargo build -v --target $(CURRENT_MUSL_TARGET) $(foreach pkg,$(MUSL_MODULE_PACKAGE_SPECS),-p $(pkg)) -p $(SYSMINION_SPEC)
	cargo build -v $(foreach pkg,$(NATIVE_REFRESH_PACKAGE_SPECS),-p $(pkg))
	$(call stage_profile_modules,debug,$(CURRENT_MUSL_TARGET))
	$(call stage_profile_minion,debug,$(CURRENT_MUSL_TARGET))
	$(call stage_modules_dist_from,debug,$(CURRENT_MUSL_TARGET),$(MUSL_MODULE_PACKAGE_SPECS))
	$(call stage_native_modules_dist,debug)
	$(call refresh_modules_repo)
	$(call refresh_current_minion_repo,debug)

modules-refresh:
	$(call tgt,wasm32-wasip1)
	@if [ -z "$(CURRENT_MUSL_TARGET)" ] || [ -z "$(CURRENT_MUSL_CC)" ]; then \
		echo "modules-refresh currently supports only configured Linux musl hosts; current host is $(UNAME_S)/$(UNAME_M)." >&2; \
		exit 1; \
	fi
	$(call tgt,$(CURRENT_MUSL_TARGET))
	$(call check_present,$(CURRENT_MUSL_CC))
	cargo build --release --target $(CURRENT_MUSL_TARGET) $(foreach pkg,$(MUSL_MODULE_PACKAGE_SPECS),-p $(pkg)) -p $(SYSMINION_SPEC)
	cargo build --release $(foreach pkg,$(NATIVE_REFRESH_PACKAGE_SPECS),-p $(pkg))
	$(call stage_profile_modules,release,$(CURRENT_MUSL_TARGET))
	$(call stage_profile_minion,release,$(CURRENT_MUSL_TARGET))
	$(call stage_modules_dist_from,release,$(CURRENT_MUSL_TARGET),$(MUSL_MODULE_PACKAGE_SPECS))
	$(call stage_native_modules_dist,release)
	$(call refresh_modules_repo)
	$(call refresh_current_minion_repo,release)

stats:
	tokei . --exclude target --exclude .git

man:
	pandoc --standalone --to man docs/manpages/sysinspect.8.md -o docs/manpages/sysinspect.8

dev-tls:
	./scripts/dev-tls.sh

tar:
	# Cleanup
	rm -rf package/${ARC_NAME}

	cargo vendor
	mkdir -p package/${ARC_NAME}/.cargo
	cp .vendor.toml package/${ARC_NAME}/.cargo/config.toml

	cp LICENSE package/${ARC_NAME}
	cp README.md package/${ARC_NAME}
	cp Cargo.lock package/${ARC_NAME}
	cp Cargo.toml package/${ARC_NAME}
	cp Makefile package/${ARC_NAME}
	cp rustfmt.toml package/${ARC_NAME}
	cp -a docs package/${ARC_NAME}
	cp -a man package/${ARC_NAME}
	cp -a libsysinspect package/${ARC_NAME}
	cp -a modules package/${ARC_NAME}
	cp -a src package/${ARC_NAME}
	cp -a vendor package/${ARC_NAME}

	# Cleanup. Also https://github.com/rust-lang/cargo/issues/7058
	find package/${ARC_NAME} -type d -wholename "*/target" -prune -exec rm -rf {} \;
	find package/${ARC_NAME} -type d -wholename "*/vendor/winapi*" -prune -exec \
		rm -rf {}/src \; -exec mkdir -p {}/src \; -exec touch {}/src/lib.rs \; -exec rm -rf {}/lib \;
	find package/${ARC_NAME} -type d -wholename "*/vendor/windows*" -prune -exec \
		rm -rf {}/src \; -exec mkdir -p {}/src \;  -exec touch {}/src/lib.rs \; -exec rm -rf {}/lib \;
	rm -rf package/${ARC_NAME}/vendor/web-sys/src/*
	rm -rf package/${ARC_NAME}/vendor/web-sys/webidls
	mkdir -p package/${ARC_NAME}/vendor/web-sys/src
	touch package/${ARC_NAME}/vendor/web-sys/src/lib.rs

	# Tar the source
	tar -C package -czvf package/${ARC_NAME}.tar.gz ${ARC_NAME}
	rm -rf package/${ARC_NAME}
	rm -rf vendor
