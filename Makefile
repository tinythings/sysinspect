.DEFAULT_GOAL := help

include Makefile.in

export PATH := $(HOME)/.cargo/bin:$(PATH)
CARGO := $(shell command -v cargo 2>/dev/null || echo cargo)

MXRUN_BIN := mxrun
MXRUN_ARGS ?=
MX_ACTIVE := $(shell awk -F= '/^active=/ {print $$2}' .mxrun-env 2>/dev/null)
C_MX  := $(if $(filter yes,$(MX_ACTIVE)),\033[1;96m,\033[1m)
C_BLD := \033[1m
C_YLW := \033[1;93m
C_GRN := \033[1;92m
C_OFF := \033[0m


.PHONY: help release mxrun mxrun-init mxrun-toggle set-local-builds set-remote-builds build dev all all-dev modules modules-dev modules-dist-dev modules-refresh-dev modules-refresh clean check fix setup smoke-test \
	stats man test test-core test-modules test-sensors test-integration tar dev-tls advisory \
	_dev _all_dev _all _build _modules_dev _modules _modules_dist_dev _test _test_core _test_modules _test_sensors _test_integration

ifeq ($(UNAME_S),Linux)
.PHONY: musl-aarch64-dev musl-aarch64 musl-x86_64-dev musl-x86_64
endif

help:
	@printf '\n$$ make [help]\n\n'
	@printf '$(C_GRN)%s$(C_OFF)\n' "Development Build"
	@printf '    $(C_YLW)%-20s$(C_OFF) %s\n' "help" "Show this help and what each entry does."
	@printf '    $(C_MX)%-20s$(C_OFF) %s\n' "dev" "Compile core binaries in development mode with debug data."
	@printf '    $(C_MX)%-20s$(C_OFF) %s\n' "all-dev" "Compile core plus modules in development mode."
	@printf '    $(C_MX)%-20s$(C_OFF) %s\n' "modules-dev" "Compile modules only in development mode."
	@printf '    $(C_MX)%-20s$(C_OFF) %s\n' "modules-dist-dev" "Build release modules and stage distribution payloads."
	@printf '    $(C_MX)%-20s$(C_OFF) %s\n' "modules-refresh-dev" "Debug variant of Linux musl module refresh."
	@printf '\n$(C_GRN)%s$(C_OFF)\n' "Release Build"
	@printf '    $(C_MX)%-20s$(C_OFF) %s\n' "release" "Compile core binaries in release mode."
	@printf '    $(C_MX)%-20s$(C_OFF) %s\n' "all" "Compile core plus modules in release mode."
	@printf '    $(C_MX)%-20s$(C_OFF) %s\n' "modules" "Compile modules only in release mode."
	@printf '    $(C_MX)%-20s$(C_OFF) %s\n' "modules-refresh" "Rebuild Linux musl module repo and refresh current minion slot."
ifeq ($(UNAME_S),Linux)
	@printf '\n$(C_GRN)%s$(C_OFF)\n' "Cross Build"
	@printf '    $(C_BLD)%-30s$(C_OFF) %s\n' "musl-x86_64" "Build static x86_64 Linux release artifacts."
	@printf '    $(C_BLD)%-30s$(C_OFF) %s\n' "musl-x86_64-dev" "Build static x86_64 Linux debug artifacts."
	@printf '    $(C_BLD)%-30s$(C_OFF) %s\n' "musl-aarch64" "Build static AArch64 Linux release artifacts."
	@printf '    $(C_BLD)%-30s$(C_OFF) %s\n' "musl-aarch64-dev" "Build static AArch64 Linux debug artifacts."
	@printf '    $(C_BLD)%-30s$(C_OFF) %s\n' "musl-x86_64-modules-dist" "Build static x86_64 Linux release modules distribution."
	@printf '    $(C_BLD)%-30s$(C_OFF) %s\n' "musl-x86_64-modules-dist-dev" "Build static x86_64 Linux debug modules distribution."
	@printf '    $(C_BLD)%-30s$(C_OFF) %s\n' "musl-aarch64-modules-dist" "Build static AArch64 Linux release modules distribution."
	@printf '    $(C_BLD)%-30s$(C_OFF) %s\n' "musl-aarch64-modules-dist-dev" "Build static AArch64 Linux debug modules distribution."
	@printf '    $(C_BLD)%-30s$(C_OFF) %s\n' "musl-x86_64" "Build static x86_64 Linux release artifacts."
	@printf '    $(C_BLD)%-30s$(C_OFF) %s\n' "musl-x86_64-dev" "Build static x86_64 Linux debug artifacts."
	@printf '    $(C_BLD)%-30s$(C_OFF) %s\n' "musl-aarch64" "Build static AArch64 Linux release artifacts."
	@printf '    $(C_BLD)%-30s$(C_OFF) %s\n' "musl-aarch64-dev" "Build static AArch64 Linux debug artifacts."
	@printf '    $(C_BLD)%-30s$(C_OFF) %s\n' "musl-x86_64-modules-dist" "Build static x86_64 Linux release modules distribution."
	@printf '    $(C_BLD)%-30s$(C_OFF) %s\n' "musl-x86_64-modules-dist-dev" "Build static x86_64 Linux debug modules distribution."
	@printf '    $(C_BLD)%-30s$(C_OFF) %s\n' "musl-aarch64-modules-dist" "Build static AArch64 Linux release modules distribution."
	@printf '    $(C_BLD)%-30s$(C_OFF) %s\n' "musl-aarch64-modules-dist-dev" "Build static AArch64 Linux debug modules distribution."
endif
	@printf '\n$(C_GRN)%s$(C_OFF)\n' "Testing"
	@printf '    $(C_MX)%-20s$(C_OFF) %s\n' "test" "Run the full nextest suite for this platform."
	@printf '    $(C_MX)%-20s$(C_OFF) %s\n' "test-core" "Run core crate unit/bin tests only."
	@printf '    $(C_MX)%-20s$(C_OFF) %s\n' "test-modules" "Run module tests only."
	@printf '    $(C_MX)%-20s$(C_OFF) %s\n' "test-sensors" "Run sensor crate tests only."
	@printf '    $(C_MX)%-20s$(C_OFF) %s\n' "test-integration" "Run integration tests only."
	@printf '\n$(C_GRN)%s$(C_OFF)\n' "Documentation"
	@printf '    $(C_YLW)%-20s$(C_OFF) %s\n' "man" "Build the sysinspect manpage from Markdown."
	@printf '\n$(C_GRN)%s$(C_OFF)\n' "Utils"
	@printf '    $(C_YLW)%-20s$(C_OFF) %s\n' "setup" "Install toolchain dependencies and Rust targets for this host."
	@printf '    $(C_YLW)%-20s$(C_OFF) %s\n' "mxrun-toggle" "Toggle mxrun availability (enable/disable remote builds)."
	@printf '    $(C_YLW)%-20s$(C_OFF) %s\n' "set-local-builds" "Disable mxrun; all builds run locally."
	@printf '    $(C_YLW)%-20s$(C_OFF) %s\n' "set-remote-builds" "Enable mxrun; builds run across the target matrix."
	@printf '    $(C_YLW)%-20s$(C_OFF) %s\n' "mxrun-init" "Validate mxrun config and initialise targets."
	@printf '    $(C_YLW)%-20s$(C_OFF) %s\n' "mxrun" "Show mxrun status (auto-inits local-only if no config found)."
	@printf '    $(C_YLW)%-20s$(C_OFF) %s\n' "smoke-test" "Run platform smoke tests."
	@printf '    $(C_YLW)%-20s$(C_OFF) %s\n' "check" "Run clippy in deny-warnings mode."
	@printf '    $(C_YLW)%-20s$(C_OFF) %s\n' "fix" "Run clippy --fix on the workspace."
	@printf '    $(C_YLW)%-20s$(C_OFF) %s\n' "advisory" "Audit dependencies for known vulnerabilities."
	@printf '    $(C_YLW)%-20s$(C_OFF) %s\n' "clean" "Remove Cargo build output."
	@printf '    $(C_YLW)%-20s$(C_OFF) %s\n' "stats" "Show code statistics via tokei."
	@printf '    $(C_YLW)%-20s$(C_OFF) %s\n' "dev-tls" "Generate local development TLS material."
	@printf '    $(C_YLW)%-20s$(C_OFF) %s\n' "tar" "Create a vendored source tarball."
	@printf '    $(C_YLW)%-20s$(C_OFF) %s\n' 'MXRUN_ARGS="..."' "Pass extra CLI flags to mxrun, e.g. --mirror-results or --mirror-root /tmp/out."
	@printf '\n$(C_MX)%s$(C_OFF)\n' "mxrun"
	@if [ "$(MX_ACTIVE)" = "yes" ]; then \
		printf '    %s\n' "Enabled — builds delegate to the target matrix."; \
		printf '    %s\n' "Use 'make mxrun-toggle' to disable."; \
	else \
		printf '    %s\n' "Disabled — builds run locally."; \
		printf '    %s\n' "Use 'make mxrun-toggle' to enable, or 'make mxrun-init' to initialise."; \
	fi
	@printf '\n'


mxrun-toggle:
	@if [ -f .mxrun-env ] && grep -q '^active=yes' .mxrun-env 2>/dev/null; then \
		sh scripts/mxrun-set-local.sh; \
	else \
		sh scripts/mxrun-set-remote.sh; \
	fi

set-local-builds:
	sh scripts/mxrun-set-local.sh

set-remote-builds:
	sh scripts/mxrun-set-remote.sh

release: build

mxrun-init: setup
	@command -v $(MXRUN_BIN) >/dev/null 2>&1 || { echo "Missing $(MXRUN_BIN). Install it first." >&2; exit 1; }
	@if [ ! -f mxrun.conf ]; then echo "local" > mxrun.conf; fi
	@printf 'active=yes\n' > .mxrun-env
	@MXRUN_CONFIG=mxrun.conf MXRUN_LOCAL_MAKE='$(MAKE)' $(MXRUN_BIN) init || true

mxrun: setup
	@command -v $(MXRUN_BIN) >/dev/null 2>&1 || { echo "Missing $(MXRUN_BIN). Install it first." >&2; exit 1; }
	@if [ ! -f mxrun.conf ] && [ ! -f .mxrun-env ]; then \
		printf 'active=no\n' > .mxrun-env; \
	fi
	sh scripts/mxrun-status.sh

setup:
	@sh scripts/setup.sh

clean:
	cargo clean

check:
ifeq ($(CI),true)
	cargo fmt --all -- --check
else
	cargo fmt --all -- --check || cargo fmt --all
endif
	cargo clippy --no-deps --workspace $(PLATFORM_WORKSPACE_EXCLUDES) -- -Dwarnings -Aunused-variables -Adead-code || \
	(cargo clippy --fix --allow-dirty --allow-staged --workspace $(PLATFORM_WORKSPACE_EXCLUDES) && \
	 cargo clippy --no-deps --workspace $(PLATFORM_WORKSPACE_EXCLUDES) -- -Dwarnings -Aunused-variables -Adead-code)

advisory:
	@cargo audit --version >/dev/null 2>&1 || { echo "Installing cargo-audit..."; cargo install cargo-audit --locked; }
	@scripts/audit-table.sh

smoke-test:
	sh smoke-tests/run.sh

ifeq ($(UNAME_S),Linux)
musl-aarch64-dev:
	@sh scripts/run-musl-cargo.sh aarch64-unknown-linux-musl aarch64-linux-musl-gcc build -v --workspace $(MUSL_WORKSPACE_EXCLUDES) --target aarch64-unknown-linux-musl
	$(call stage_profile_modules,debug,aarch64-unknown-linux-musl)
	$(call stage_profile_minion,debug,aarch64-unknown-linux-musl)

musl-aarch64:
	@sh scripts/run-musl-cargo.sh aarch64-unknown-linux-musl aarch64-linux-musl-gcc build --release --workspace $(MUSL_WORKSPACE_EXCLUDES) --target aarch64-unknown-linux-musl
	$(call stage_profile_modules,release,aarch64-unknown-linux-musl)
	$(call stage_profile_minion,release,aarch64-unknown-linux-musl)

musl-x86_64-dev:
	@sh scripts/run-musl-cargo.sh x86_64-unknown-linux-musl x86_64-linux-musl-gcc build -v --workspace $(MUSL_WORKSPACE_EXCLUDES) --target x86_64-unknown-linux-musl
	$(call stage_profile_modules,debug,x86_64-unknown-linux-musl)
	$(call stage_profile_minion,debug,x86_64-unknown-linux-musl)

musl-x86_64:
	@sh scripts/run-musl-cargo.sh x86_64-unknown-linux-musl x86_64-linux-musl-gcc build --release --workspace $(MUSL_WORKSPACE_EXCLUDES) --target x86_64-unknown-linux-musl
	$(call stage_profile_modules,release,x86_64-unknown-linux-musl)
	$(call stage_profile_minion,release,x86_64-unknown-linux-musl)
endif

musl-x86_64-modules-dist-dev:
	$(call check_present,x86_64-linux-musl-gcc)
	@sh scripts/run-musl-cargo.sh x86_64-unknown-linux-musl x86_64-linux-musl-gcc build -v --workspace $(MUSL_WORKSPACE_EXCLUDES) --target x86_64-unknown-linux-musl
	$(call stage_profile_modules,debug,x86_64-unknown-linux-musl)
	$(call stage_modules_dist_from,debug,x86_64-unknown-linux-musl,$(MUSL_MODULE_PACKAGE_SPECS),$(call musl_modules_dist_dir,x86_64,debug))

musl-x86_64-modules-dist:
	$(call check_present,x86_64-linux-musl-gcc)
	@sh scripts/run-musl-cargo.sh x86_64-unknown-linux-musl x86_64-linux-musl-gcc build --release --workspace $(MUSL_WORKSPACE_EXCLUDES) --target x86_64-unknown-linux-musl
	$(call stage_profile_modules,release,x86_64-unknown-linux-musl)
	$(call stage_modules_dist_from,release,x86_64-unknown-linux-musl,$(MUSL_MODULE_PACKAGE_SPECS),$(call musl_modules_dist_dir,x86_64,release))

musl-aarch64-modules-dist-dev:
	$(call check_present,aarch64-linux-musl-gcc)
	@sh scripts/run-musl-cargo.sh aarch64-unknown-linux-musl aarch64-linux-musl-gcc build -v --workspace $(MUSL_WORKSPACE_EXCLUDES) --target aarch64-unknown-linux-musl
	$(call stage_profile_modules,debug,aarch64-unknown-linux-musl)
	$(call stage_modules_dist_from,debug,aarch64-unknown-linux-musl,$(MUSL_MODULE_PACKAGE_SPECS),$(call musl_modules_dist_dir,aarch64,debug))

musl-aarch64-modules-dist:
	$(call check_present,aarch64-linux-musl-gcc)
	@sh scripts/run-musl-cargo.sh aarch64-unknown-linux-musl aarch64-linux-musl-gcc build --release --workspace $(MUSL_WORKSPACE_EXCLUDES) --target aarch64-unknown-linux-musl
	$(call stage_profile_modules,release,aarch64-unknown-linux-musl)
	$(call stage_modules_dist_from,release,aarch64-unknown-linux-musl,$(MUSL_MODULE_PACKAGE_SPECS),$(call musl_modules_dist_dir,aarch64,release))

all-dev:
	@scripts/maybe-mxrun.sh all-dev || $(MAKE) _all_dev

_all_dev:
	cargo build -v --workspace $(PLATFORM_WORKSPACE_EXCLUDES)
	$(call stage_profile_modules,debug,)
	$(call stage_profile_minion,debug,)
	$(call write_mxrun_manifest,all-dev,)

all:
	@scripts/maybe-mxrun.sh all || $(MAKE) _all

_all:
	cargo build --release --workspace $(PLATFORM_WORKSPACE_EXCLUDES)
	$(call stage_profile_modules,release,)
	$(call stage_profile_minion,release,)
	$(call write_mxrun_manifest,all,)

dev:
	@scripts/maybe-mxrun.sh dev || $(MAKE) _dev

_dev:
	cargo build -v --workspace $(CORE_EXCLUDES)
	$(call stage_profile_modules,debug,)
	$(call stage_profile_minion,debug,)
	$(call write_mxrun_manifest,dev,)

build:
	@scripts/maybe-mxrun.sh release || $(MAKE) _build

_build:
	cargo build --release --workspace $(CORE_EXCLUDES)
	$(call stage_profile_modules,release,)
	$(call stage_profile_minion,release,)
	$(call write_mxrun_manifest,release,)

modules-dev:
	@scripts/maybe-mxrun.sh modules-dev || $(MAKE) _modules_dev

_modules_dev:
	@CARGO_BUILD_JOBS=$(MODULE_BUILD_JOBS) cargo build -v $(foreach pkg,$(MODULE_PACKAGE_SPECS),-p $(pkg))
	$(call stage_profile_modules,debug,)
	$(call write_mxrun_manifest,modules-dev,)

modules:
	@scripts/maybe-mxrun.sh modules || $(MAKE) _modules

_modules:
	@CARGO_BUILD_JOBS=$(MODULE_BUILD_JOBS) cargo build --release $(foreach pkg,$(MODULE_PACKAGE_SPECS),-p $(pkg))
	$(call stage_profile_modules,release,)
	$(call write_mxrun_manifest,modules,)

modules-dist-dev:
	@scripts/maybe-mxrun.sh modules-dist-dev || $(MAKE) _modules_dist_dev

_modules_dist_dev:
	@CARGO_BUILD_JOBS=$(MODULE_BUILD_JOBS) cargo build --release $(foreach pkg,$(MODULE_PACKAGE_SPECS),-p $(pkg))
	$(call stage_profile_modules,release,)
	$(call stage_modules_dist)
	$(call write_mxrun_manifest,modules-dist-dev,with-dist)

test: setup
	@scripts/maybe-mxrun.sh test || $(MAKE) _test

_test:
	@CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run --no-fail-fast --workspace $(PLATFORM_WORKSPACE_EXCLUDES) --test-threads $(TEST_RUN_THREADS)

test-core: setup
	@scripts/maybe-mxrun.sh test-core || $(MAKE) _test_core

_test_core:
	@CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run --no-fail-fast $(foreach pkg,$(CORE_PACKAGE_SPECS),-p $(pkg)) --lib --bins --test-threads $(TEST_RUN_THREADS)

test-modules: setup
	@scripts/maybe-mxrun.sh test-modules || $(MAKE) _test_modules

_test_modules:
	@CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run --no-fail-fast $(foreach pkg,$(MODULE_PACKAGE_SPECS),-p $(pkg)) --bins --test-threads $(TEST_RUN_THREADS)

test-sensors: setup
	@scripts/maybe-mxrun.sh test-sensors || $(MAKE) _test_sensors

_test_sensors:
	@CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run --no-fail-fast $(foreach pkg,$(SENSOR_PACKAGE_SPECS),-p $(pkg)) --lib --bins --test-threads $(TEST_RUN_THREADS)

test-integration: setup
	@scripts/maybe-mxrun.sh test-integration || $(MAKE) _test_integration

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
