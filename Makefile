.DEFAULT_GOAL := build

include Makefile.in

.PHONY: build devel all all-devel modules modules-dev modules-dist-devel modules-refresh-devel modules-refresh clean check fix setup \
	musl-aarch64-dev musl-aarch64 musl-x86_64-dev musl-x86_64 \
	stats man test test-core test-modules test-sensors test-integration tar dev-tls

setup:
	$(call deps)
	$(call tgt,wasm32-wasip1)
	$(call tgt,aarch64-unknown-linux-musl)
	$(call tgt,x86_64-unknown-linux-musl)

clean:
	cargo clean

check:
	cargo clippy --no-deps --workspace $(PLATFORM_WORKSPACE_EXCLUDES) -- -Dwarnings -Aunused-variables -Adead-code

fix:
	cargo clippy --fix --allow-dirty --allow-staged --workspace $(PLATFORM_WORKSPACE_EXCLUDES)

musl-aarch64-dev:
	$(call check_present,aarch64-linux-musl-gcc)
	$(call prep_layout,debug,aarch64-unknown-linux-musl)
	cargo build -v --workspace $(MUSL_WORKSPACE_EXCLUDES) --target aarch64-unknown-linux-musl
	$(call move_bin,debug,aarch64-unknown-linux-musl)

musl-aarch64:
	$(call check_present,aarch64-linux-musl-gcc)
	$(call prep_layout,release,aarch64-unknown-linux-musl)
	cargo build --release --workspace $(MUSL_WORKSPACE_EXCLUDES) --target aarch64-unknown-linux-musl
	$(call move_bin,release,aarch64-unknown-linux-musl)

musl-x86_64-dev:
	$(call check_present,x86_64-linux-musl-gcc)
	$(call prep_layout,debug,x86_64-unknown-linux-musl)
	cargo build -v --workspace $(MUSL_WORKSPACE_EXCLUDES) --target x86_64-unknown-linux-musl
	$(call move_bin,debug,x86_64-unknown-linux-musl)

musl-x86_64:
	$(call check_present,x86_64-linux-musl-gcc)
	$(call prep_layout,release,x86_64-unknown-linux-musl)
	cargo build --release --workspace $(MUSL_WORKSPACE_EXCLUDES) --target x86_64-unknown-linux-musl
	$(call move_bin,release,x86_64-unknown-linux-musl)

all-devel:
	$(call prep_layout,debug,)
	cargo build -v --workspace $(PLATFORM_WORKSPACE_EXCLUDES)
	$(call move_bin,debug,)

all:
	$(call prep_layout,release,)
	cargo build --release --workspace $(PLATFORM_WORKSPACE_EXCLUDES)
	$(call move_bin,release,)

devel:
	$(call prep_layout,debug,)
	cargo build -v --workspace $(CORE_EXCLUDES)
	$(call move_bin,debug,)

build:
	$(call prep_layout,release,)
	cargo build --release --workspace $(CORE_EXCLUDES)
	$(call move_bin,release,)

modules-dev:
	$(call prep_layout,debug,)
	cargo build -v $(foreach pkg,$(MODULE_PACKAGE_SPECS),-p $(pkg))
	$(call move_bin,debug,)

modules:
	$(call prep_layout,release,)
	cargo build --release $(foreach pkg,$(MODULE_PACKAGE_SPECS),-p $(pkg))
	$(call move_bin,release,)

modules-dist-devel:
	$(call prep_layout,release,)
	cargo build --release $(foreach pkg,$(MODULE_PACKAGE_SPECS),-p $(pkg))
	$(call stage_modules_dist)

modules-refresh-devel:
	$(call tgt,wasm32-wasip1)
	@if [ -z "$(CURRENT_MUSL_TARGET)" ] || [ -z "$(CURRENT_MUSL_CC)" ]; then \
		echo "modules-refresh-devel currently supports only configured Linux musl hosts; current host is $(UNAME_S)/$(UNAME_M)." >&2; \
		exit 1; \
	fi
	$(call tgt,$(CURRENT_MUSL_TARGET))
	$(call check_present,$(CURRENT_MUSL_CC))
	$(call prep_layout,debug,$(CURRENT_MUSL_TARGET))
	cargo build -v --target $(CURRENT_MUSL_TARGET) $(foreach pkg,$(MUSL_MODULE_PACKAGE_SPECS),-p $(pkg)) -p $(SYSMINION_SPEC)
	cargo build -v $(foreach pkg,$(NATIVE_REFRESH_PACKAGE_SPECS),-p $(pkg))
	$(call stage_modules_dist_from,debug,$(CURRENT_MUSL_TARGET))
	$(call stage_native_modules_dist,debug)
	$(call refresh_modules_repo)
	$(call refresh_current_minion_repo)

modules-refresh:
	$(call tgt,wasm32-wasip1)
	@if [ -z "$(CURRENT_MUSL_TARGET)" ] || [ -z "$(CURRENT_MUSL_CC)" ]; then \
		echo "modules-refresh currently supports only configured Linux musl hosts; current host is $(UNAME_S)/$(UNAME_M)." >&2; \
		exit 1; \
	fi
	$(call tgt,$(CURRENT_MUSL_TARGET))
	$(call check_present,$(CURRENT_MUSL_CC))
	$(call prep_layout,release,$(CURRENT_MUSL_TARGET))
	cargo build --release --target $(CURRENT_MUSL_TARGET) $(foreach pkg,$(MUSL_MODULE_PACKAGE_SPECS),-p $(pkg)) -p $(SYSMINION_SPEC)
	cargo build --release $(foreach pkg,$(NATIVE_REFRESH_PACKAGE_SPECS),-p $(pkg))
	$(call stage_modules_dist_from,release,$(CURRENT_MUSL_TARGET))
	$(call stage_native_modules_dist,release)
	$(call refresh_modules_repo)
	@sysbin=target/debug/sysinspect; \
	if [ ! -x "$$sysbin" ]; then \
		echo "Building debug sysinspect helper"; \
		cargo build -v -p $(SYSINSPECT_SPEC); \
	fi; \
	minion=target/$(CURRENT_MUSL_TARGET)/release/sysminion; \
	if [ ! -x "$$minion" ]; then \
		echo "Missing built minion binary: $$minion" >&2; \
		exit 1; \
	fi; \
	"$$sysbin" module -R -t --name $(CURRENT_MINION_SLOT) || true; \
	"$$sysbin" module -A -t -p "$$minion"

stats:
	tokei . --exclude target --exclude .git

man:
	pandoc --standalone --to man docs/manpages/sysinspect.8.md -o docs/manpages/sysinspect.8

dev-tls:
	./scripts/dev-tls.sh

test: setup
	@CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run --workspace $(PLATFORM_WORKSPACE_EXCLUDES) --test-threads $(TEST_RUN_THREADS)
test-core: setup
	@CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run $(foreach pkg,$(CORE_PACKAGE_SPECS),-p $(pkg)) --lib --bins --test-threads $(TEST_RUN_THREADS)

test-modules: setup
	@CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run $(foreach pkg,$(MODULE_PACKAGE_SPECS),-p $(pkg)) --bins --test-threads $(TEST_RUN_THREADS)

test-sensors: setup
	@CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run $(foreach pkg,$(SENSOR_PACKAGE_SPECS),-p $(pkg)) --lib --bins --test-threads $(TEST_RUN_THREADS)

test-integration: setup
	@CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run $(INTEGRATION_TEST_TARGETS) --test-threads $(TEST_RUN_THREADS)

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
