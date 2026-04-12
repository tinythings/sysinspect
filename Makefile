.DEFAULT_GOAL := help

include Makefile.in

BUILDFARM_BIN := target/buildfarm/buildfarm
BUILDFARM_ARGS ?=

.PHONY: help release buildfarm buildfarm-init build dev all all-dev modules modules-dev modules-dist-dev modules-refresh-dev modules-refresh clean check fix setup smoke-test \
	musl-aarch64-dev musl-aarch64 musl-x86_64-dev musl-x86_64 \
	stats man test test-core test-modules test-sensors test-integration tar dev-tls

help:
	@printf '\n$$ make [help]\n\n'
	@printf '\033[1;92m%s\033[0m\n' "Development"
	@printf '    \033[1;93m%-20s\033[0m %s\n' "help" "Show this help and what each entry does."
	@printf '    %b\033[1;93m%-19s\033[0m %s\n' '$(if $(strip $(BUILDFARM_CONFIG)),\033[1;91m*\033[0m,)' "dev" "Compile core binaries in development mode with debug data."
	@printf '    %b\033[1;93m%-19s\033[0m %s\n' '$(if $(strip $(BUILDFARM_CONFIG)),\033[1;91m*\033[0m,)' "all-dev" "Compile core plus modules in development mode."
	@printf '    %b\033[1;93m%-19s\033[0m %s\n' '$(if $(strip $(BUILDFARM_CONFIG)),\033[1;91m*\033[0m,)' "modules-dev" "Compile modules only in development mode."
	@printf '    %b\033[1;93m%-19s\033[0m %s\n' '$(if $(strip $(BUILDFARM_CONFIG)),\033[1;91m*\033[0m,)' "modules-dist-dev" "Build release modules and stage distribution payloads."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "modules-refresh-dev" "Debug variant of Linux musl module refresh."
	@printf '\n\033[1;92m%s\033[0m\n' "Release"
	@printf '    %b\033[1;93m%-19s\033[0m %s\n' '$(if $(strip $(BUILDFARM_CONFIG)),\033[1;91m*\033[0m,)' "release" "Compile core binaries in release mode."
	@printf '    %b\033[1;93m%-19s\033[0m %s\n' '$(if $(strip $(BUILDFARM_CONFIG)),\033[1;91m*\033[0m,)' "all" "Compile core plus modules in release mode."
	@printf '    %b\033[1;93m%-19s\033[0m %s\n' '$(if $(strip $(BUILDFARM_CONFIG)),\033[1;91m*\033[0m,)' "modules" "Compile modules only in release mode."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "modules-refresh" "Rebuild Linux musl module repo and refresh current minion slot."
	@printf '\n\033[1;92m%s\033[0m\n' "Utils"
	@printf '    \033[1;93m%-20s\033[0m %s\n' "setup" "Install toolchain dependencies and Rust targets for this host."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "buildfarm" "Compile the standalone buildfarm controller into target/buildfarm/."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "buildfarm-init" "Reset remote destinations from BUILDFARM_CONFIG and sync project contents."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "smoke-test" "Run platform smoke tests."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "check" "Run clippy in deny-warnings mode."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "fix" "Run clippy --fix on the workspace."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "clean" "Remove Cargo build output."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "stats" "Show code statistics via tokei."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "dev-tls" "Generate local development TLS material."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "tar" "Create a vendored source tarball."
	@printf '    \033[1;93m%-20s\033[0m %s\n' 'BUILDFARM_ARGS="..."' "Pass extra CLI flags to buildfarm, e.g. --mirror-results or --mirror-root /tmp/out."
	@printf '\n\033[1;92m%s\033[0m\n' "Testing"
	@printf '    \033[1;93m%-20s\033[0m %s\n' "test" "Run the full nextest suite for this platform."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "test-core" "Run core crate unit/bin tests only."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "test-modules" "Run module tests only."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "test-sensors" "Run sensor crate tests only."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "test-integration" "Run integration tests only."
	@printf '\n\033[1;92m%s\033[0m\n' "Cross Builds"
	@printf '    \033[1;93m%-20s\033[0m %s\n' "musl-x86_64" "Build static x86_64 Linux release artifacts."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "musl-x86_64-dev" "Build static x86_64 Linux debug artifacts."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "musl-aarch64" "Build static AArch64 Linux release artifacts."
	@printf '    \033[1;93m%-20s\033[0m %s\n' "musl-aarch64-dev" "Build static AArch64 Linux debug artifacts."
	@printf '\n\033[1;92m%s\033[0m\n' "Documentation"
	@printf '    \033[1;93m%-20s\033[0m %s\n' "man" "Build the sysinspect manpage from Markdown."
	@if [ -n "$(BUILDFARM_CONFIG)" ]; then \
		printf '\n\033[1;96m%s\033[0m\n' "Legend"; \
		printf '    \033[1;91m*\033[0m\033[1;93m%-19s\033[0m %s\n' "entry" "Runs across the buildfarm defined by BUILDFARM_CONFIG."; \
	else \
		printf '\n\033[1;96m%s\033[0m\n' "Buildfarm"; \
		printf '    %s\n' "In order to activate buildfarm mode, export the following environment:"; \
		printf '        %s\n' "export BUILDFARM_CONFIG=<buildfarm.conf file>"; \
	fi
	@printf '\n'

release: build

buildfarm-init: setup
	@BUILDFARM_CONFIG='$(BUILDFARM_CONFIG)' BUILDFARM_LOCAL_MAKE='$(MAKE)' sh scripts/buildfarm.sh init

define buildfarm_compile
	@rm -rf target/buildfarm buildfarm/target
	@mkdir -p target/buildfarm
	@cargo build --manifest-path buildfarm/Cargo.toml --target-dir buildfarm/target
	@cp -f buildfarm/target/debug/buildfarm $(BUILDFARM_BIN)
	@chmod +x $(BUILDFARM_BIN)
	@rm -rf buildfarm/target
endef

buildfarm: setup
	$(buildfarm_compile)

$(BUILDFARM_BIN):
	$(buildfarm_compile)

setup:
	$(call deps)
	$(call setup_targets)
	@if [ ! -x "$(BUILDFARM_BIN)" ]; then \
		echo "Prebuilding standalone buildfarm controller"; \
		rm -rf target/buildfarm buildfarm/target; \
		mkdir -p target/buildfarm; \
		cargo build --manifest-path buildfarm/Cargo.toml --target-dir buildfarm/target; \
		cp -f buildfarm/target/debug/buildfarm $(BUILDFARM_BIN); \
		chmod +x $(BUILDFARM_BIN); \
		rm -rf buildfarm/target; \
	fi

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

ifneq ($(strip $(BUILDFARM_CONFIG)),)
all-dev:
	@[ -x "$(BUILDFARM_BIN)" ] || { echo "Missing $(BUILDFARM_BIN). Run 'make setup' or 'make buildfarm' first." >&2; exit 1; }
	@BUILDFARM_CONFIG='$(BUILDFARM_CONFIG)' BUILDFARM_LOCAL_MAKE='$(MAKE)' $(BUILDFARM_BIN) run all-dev $(BUILDFARM_ARGS)

all:
	@[ -x "$(BUILDFARM_BIN)" ] || { echo "Missing $(BUILDFARM_BIN). Run 'make setup' or 'make buildfarm' first." >&2; exit 1; }
	@BUILDFARM_CONFIG='$(BUILDFARM_CONFIG)' BUILDFARM_LOCAL_MAKE='$(MAKE)' $(BUILDFARM_BIN) run all $(BUILDFARM_ARGS)

dev:
	@[ -x "$(BUILDFARM_BIN)" ] || { echo "Missing $(BUILDFARM_BIN). Run 'make setup' or 'make buildfarm' first." >&2; exit 1; }
	@BUILDFARM_CONFIG='$(BUILDFARM_CONFIG)' BUILDFARM_LOCAL_MAKE='$(MAKE)' $(BUILDFARM_BIN) run dev $(BUILDFARM_ARGS)

build:
	@[ -x "$(BUILDFARM_BIN)" ] || { echo "Missing $(BUILDFARM_BIN). Run 'make setup' or 'make buildfarm' first." >&2; exit 1; }
	@BUILDFARM_CONFIG='$(BUILDFARM_CONFIG)' BUILDFARM_LOCAL_MAKE='$(MAKE)' $(BUILDFARM_BIN) run release $(BUILDFARM_ARGS)

modules-dev:
	@[ -x "$(BUILDFARM_BIN)" ] || { echo "Missing $(BUILDFARM_BIN). Run 'make setup' or 'make buildfarm' first." >&2; exit 1; }
	@BUILDFARM_CONFIG='$(BUILDFARM_CONFIG)' BUILDFARM_LOCAL_MAKE='$(MAKE)' $(BUILDFARM_BIN) run modules-dev $(BUILDFARM_ARGS)

modules:
	@[ -x "$(BUILDFARM_BIN)" ] || { echo "Missing $(BUILDFARM_BIN). Run 'make setup' or 'make buildfarm' first." >&2; exit 1; }
	@BUILDFARM_CONFIG='$(BUILDFARM_CONFIG)' BUILDFARM_LOCAL_MAKE='$(MAKE)' $(BUILDFARM_BIN) run modules $(BUILDFARM_ARGS)

modules-dist-dev:
	@[ -x "$(BUILDFARM_BIN)" ] || { echo "Missing $(BUILDFARM_BIN). Run 'make setup' or 'make buildfarm' first." >&2; exit 1; }
	@BUILDFARM_CONFIG='$(BUILDFARM_CONFIG)' BUILDFARM_LOCAL_MAKE='$(MAKE)' $(BUILDFARM_BIN) run modules-dist-dev $(BUILDFARM_ARGS)
else
all-dev:
	cargo build -v --workspace $(PLATFORM_WORKSPACE_EXCLUDES)
	$(call stage_profile_modules,debug,)
	$(call stage_profile_minion,debug,)
	$(call write_buildfarm_manifest,all-dev,)

all:
	cargo build --release --workspace $(PLATFORM_WORKSPACE_EXCLUDES)
	$(call stage_profile_modules,release,)
	$(call stage_profile_minion,release,)
	$(call write_buildfarm_manifest,all,)

dev:
	cargo build -v --workspace $(CORE_EXCLUDES)
	$(call stage_profile_modules,debug,)
	$(call stage_profile_minion,debug,)
	$(call write_buildfarm_manifest,dev,)

build:
	cargo build --release --workspace $(CORE_EXCLUDES)
	$(call stage_profile_modules,release,)
	$(call stage_profile_minion,release,)
	$(call write_buildfarm_manifest,release,)

modules-dev:
	@CARGO_BUILD_JOBS=$(MODULE_BUILD_JOBS) cargo build -v $(foreach pkg,$(MODULE_PACKAGE_SPECS),-p $(pkg))
	$(call stage_profile_modules,debug,)
	$(call write_buildfarm_manifest,modules-dev,)

modules:
	@CARGO_BUILD_JOBS=$(MODULE_BUILD_JOBS) cargo build --release $(foreach pkg,$(MODULE_PACKAGE_SPECS),-p $(pkg))
	$(call stage_profile_modules,release,)
	$(call write_buildfarm_manifest,modules,)

modules-dist-dev:
	@CARGO_BUILD_JOBS=$(MODULE_BUILD_JOBS) cargo build --release $(foreach pkg,$(MODULE_PACKAGE_SPECS),-p $(pkg))
	$(call stage_profile_modules,release,)
	$(call stage_modules_dist)
	$(call write_buildfarm_manifest,modules-dist-dev,with-dist)
endif

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

test: setup
	@CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run --no-fail-fast --workspace $(PLATFORM_WORKSPACE_EXCLUDES) --test-threads $(TEST_RUN_THREADS)
test-core: setup
	@CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run --no-fail-fast $(foreach pkg,$(CORE_PACKAGE_SPECS),-p $(pkg)) --lib --bins --test-threads $(TEST_RUN_THREADS)

test-modules: setup
	@CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run --no-fail-fast $(foreach pkg,$(MODULE_PACKAGE_SPECS),-p $(pkg)) --bins --test-threads $(TEST_RUN_THREADS)

test-sensors: setup
	@CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run --no-fail-fast $(foreach pkg,$(SENSOR_PACKAGE_SPECS),-p $(pkg)) --lib --bins --test-threads $(TEST_RUN_THREADS)

test-integration: setup
	@CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run --no-fail-fast $(INTEGRATION_TEST_TARGETS) --test-threads $(TEST_RUN_THREADS)

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
