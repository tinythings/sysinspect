.DEFAULT_GOAL := build

ARC_VERSION := $(shell cat src/main.rs | grep 'static VERSION' | sed -e 's/.*=//g' -e 's/[" ;]//g')
ARC_NAME := sysinspect-${ARC_VERSION}
PACK_LAYOUT_DIRS := sys net fs runtime cfg
PKG_SPEC_FROM_TOML = $(shell awk 'BEGIN{name=""; version=""} /^name = / && name == "" { gsub(/"/, "", $$3); name = $$3 } /^version = / && version == "" { gsub(/"/, "", $$3); version = $$3 } END { if (name != "" && version != "") printf "%s@%s", name, version }' $(1))
MODULE_PACKAGE_SPECS := $(shell find modules -maxdepth 3 -name Cargo.toml -print | sort | while read f; do \
	awk 'BEGIN{name=""; version=""} \
		/^name = / && name == "" { gsub(/"/, "", $$3); name = $$3 } \
		/^version = / && version == "" { gsub(/"/, "", $$3); version = $$3 } \
		END { if (name != "" && version != "") printf "%s@%s ", name, version }' "$$f"; \
done)
SENSOR_PACKAGE_SPECS := $(foreach f,libsensors/Cargo.toml libmenotify/Cargo.toml,$(call PKG_SPEC_FROM_TOML,$(f)))
CORE_PACKAGE_SPECS := $(strip \
	$(call PKG_SPEC_FROM_TOML,Cargo.toml) \
	$(call PKG_SPEC_FROM_TOML,libsysinspect/Cargo.toml) \
	$(call PKG_SPEC_FROM_TOML,libeventreg/Cargo.toml) \
	$(call PKG_SPEC_FROM_TOML,sysmaster/Cargo.toml) \
	$(call PKG_SPEC_FROM_TOML,sysminion/Cargo.toml) \
	$(call PKG_SPEC_FROM_TOML,libsetup/Cargo.toml) \
	$(call PKG_SPEC_FROM_TOML,libscheduler/Cargo.toml) \
	$(call PKG_SPEC_FROM_TOML,libmodpak/Cargo.toml) \
	$(call PKG_SPEC_FROM_TOML,libmodcore/Cargo.toml) \
	$(call PKG_SPEC_FROM_TOML,libtelemetry/Cargo.toml) \
	$(call PKG_SPEC_FROM_TOML,libwebapi/Cargo.toml) \
	$(call PKG_SPEC_FROM_TOML,sysclient/Cargo.toml) \
	$(call PKG_SPEC_FROM_TOML,libdpq/Cargo.toml) \
	$(call PKG_SPEC_FROM_TOML,libsysproto/Cargo.toml) \
	$(call PKG_SPEC_FROM_TOML,libcommon/Cargo.toml) \
	$(call PKG_SPEC_FROM_TOML,libdatastore/Cargo.toml))
INTEGRATION_TEST_TARGETS := $(shell find . -path '*/tests/*.rs' | sort | while read f; do \
	dir=$$(dirname "$$f"); \
	base=$$(basename "$$f" .rs); \
	crate_dir=$$(dirname "$$dir"); \
	if [ -f "$$crate_dir/Cargo.toml" ]; then \
		spec=$$(awk 'BEGIN{name=""; version=""} \
			/^name = / && name == "" { gsub(/"/, "", $$3); name = $$3 } \
			/^version = / && version == "" { gsub(/"/, "", $$3); version = $$3 } \
			END { if (name != "" && version != "") printf "%s@%s", name, version }' "$$crate_dir/Cargo.toml"); \
		if [ -n "$$spec" ]; then printf -- "-p %s --test %s " "$$spec" "$$base"; fi; \
	fi; \
done)
INTEGRATION_TEST_TARGETS += -p $(call PKG_SPEC_FROM_TOML,libmenotify/Cargo.toml) --test githubissues_demo_it
CORE_EXCLUDES := $(foreach pkg,$(MODULE_PACKAGE_SPECS),--exclude $(pkg))
TEST_BUILD_JOBS ?= $(shell sh -c 'n=$$(command -v nproc >/dev/null 2>&1 && nproc || sysctl -n hw.ncpu 2>/dev/null || echo 2); if [ "$$n" -gt 2 ]; then echo $$((($$n + 1) / 2)); else echo 1; fi')
TEST_RUN_THREADS ?= 3

.PHONY: build devel all all-devel modules modules-dev clean check fix setup \
	musl-aarch64-dev musl-aarch64 musl-x86_64-dev musl-x86_64 \
	stats man test test-core test-modules test-sensors test-integration tar

define deps
	@OS_ID=$$(. /etc/os-release 2>/dev/null && echo $$ID || lsb_release -si 2>/dev/null); \
	SUDO=$$(command -v sudo >/dev/null 2>&1 && echo sudo || true); \
	if [ "$$OS_ID" = "Ubuntu" ] || [ "$$OS_ID" = "Debian" ]; then \
		echo "Installing required packages: pkg-config, libssl-dev, libffi-dev"; \
		$$SUDO apt-get update && $$SUDO apt-get install -y pkg-config libssl-dev libffi-dev libsodium-dev libpam0g-dev; \
		$$SUDO apt-get install -y llvm-dev libclang-dev clang; \
		$$SUDO apt-get install -y protobuf-compiler; \
	else \
		echo "Oops, no fun for $$OS_ID right now. Builds are only possible on Debian/Ubuntu." >&2; \
		echo "But! You can fix this by sending your PR here: https://github.com/tinythings/sysinspect :-)" >&2; \
		exit 1; \
	fi; \
	cargo nextest --version >/dev/null 2>&1 || cargo install cargo-nextest --locked; \
	command -v tokei >/dev/null 2>&1 || cargo install tokei --locked
endef

define tgt
	@t=$(1); \
	echo "Adding target $$t"; \
	rustup target add $$t;
endef

define check_present
	@if ! command -v $(1) >/dev/null 2>&1; then \
		echo "Error: '$(1)' is not found, aborting." >&2; \
		exit 1; \
	fi
endef

define prep_layout
	@dir=$$(if [ -n "$(2)" ]; then echo target/$(2)/$(1); else echo target/$(1); fi); \
	for layout in $(PACK_LAYOUT_DIRS); do \
		if [ -d $$dir/$$layout ]; then rm -rf $$dir/$$layout; fi; \
	done
endef


define move_bin
	@dir=$$(if [ -n "$(2)" ]; then echo target/$(2)/$(1); else echo target/$(1); fi); \
	echo "Moving binaries in $$dir"; \
	if [ -f $$dir/net ]; then mv $$dir/net $$dir/.net.bin; fi; \
	if [ -f $$dir/http ]; then mv $$dir/http $$dir/.http.bin; fi; \
	rm -rf $$dir/sys; \
	mkdir -p $$dir/sys; \
	rm -rf $$dir/net; \
	mkdir -p $$dir/net; \
	rm -rf $$dir/fs; \
	mkdir -p $$dir/fs; \
	rm -rf $$dir/runtime; \
	mkdir -p $$dir/runtime; \
	rm -rf $$dir/cfg; \
	mkdir -p $$dir/cfg; \
	if [ -f $$dir/proc ]; then mv $$dir/proc $$dir/sys/; fi; \
	if [ -f $$dir/.net.bin ]; then mv $$dir/.net.bin $$dir/sys/net; fi; \
	if [ -f $$dir/run ]; then mv $$dir/run $$dir/sys/; fi; \
	if [ -f $$dir/ssrun ]; then mv $$dir/ssrun $$dir/sys/; fi; \
	if [ -f $$dir/.http.bin ]; then mv $$dir/.http.bin $$dir/net/http; fi; \
	if [ -f $$dir/file ]; then mv $$dir/file $$dir/fs/; fi; \
	if [ -f $$dir/lua-runtime ]; then mv $$dir/lua-runtime $$dir/runtime/; fi; \
	if [ -f $$dir/py3-runtime ]; then mv $$dir/py3-runtime $$dir/runtime/; fi; \
	if [ -f $$dir/wasm-runtime ]; then mv $$dir/wasm-runtime $$dir/runtime/; fi; \
	if [ -f $$dir/resource ]; then mv $$dir/resource $$dir/cfg/; fi;
endef

setup:
	$(call deps)
	$(call tgt,wasm32-wasip1)
	$(call tgt,aarch64-unknown-linux-musl)
	$(call tgt,x86_64-unknown-linux-musl)

clean:
	cargo clean

check:
	cargo clippy --no-deps --all -- -Dwarnings -Aunused-variables -Adead-code

fix:
	cargo clippy --fix --allow-dirty --allow-staged --all

musl-aarch64-dev:
	$(call check_present,aarch64-linux-musl-gcc)
	$(call prep_layout,debug,aarch64-unknown-linux-musl)
	cargo build -v --workspace --target aarch64-unknown-linux-musl
	$(call move_bin,debug,aarch64-unknown-linux-musl)

musl-aarch64:
	$(call check_present,aarch64-linux-musl-gcc)
	$(call prep_layout,release,aarch64-unknown-linux-musl)
	cargo build --release --workspace --target aarch64-unknown-linux-musl
	$(call move_bin,release,aarch64-unknown-linux-musl)

musl-x86_64-dev:
	$(call check_present,x86_64-linux-musl-gcc)
	$(call prep_layout,debug,x86_64-unknown-linux-musl)
	cargo build -v --workspace --target x86_64-unknown-linux-musl
	$(call move_bin,debug,x86_64-unknown-linux-musl)

musl-x86_64:
	$(call check_present,x86_64-linux-musl-gcc)
	$(call prep_layout,release,x86_64-unknown-linux-musl)
	cargo build --release --workspace --target x86_64-unknown-linux-musl
	$(call move_bin,release,x86_64-unknown-linux-musl)

all-devel:
	$(call prep_layout,debug,)
	cargo build -v --workspace
	$(call move_bin,debug,)

all:
	$(call prep_layout,release,)
	cargo build --release --workspace
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

stats:
	tokei . --exclude target --exclude .git

man:
	pandoc --standalone --to man docs/manpages/sysinspect.8.md -o docs/manpages/sysinspect.8

test: setup
	CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run --workspace --test-threads $(TEST_RUN_THREADS)

test-core: setup
	CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run $(foreach pkg,$(CORE_PACKAGE_SPECS),-p $(pkg)) --lib --bins --test-threads $(TEST_RUN_THREADS)

test-modules: setup
	CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run $(foreach pkg,$(MODULE_PACKAGE_SPECS),-p $(pkg)) --bins --test-threads $(TEST_RUN_THREADS)

test-sensors: setup
	CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run $(foreach pkg,$(SENSOR_PACKAGE_SPECS),-p $(pkg)) --lib --bins --test-threads $(TEST_RUN_THREADS)

test-integration: setup
	CARGO_BUILD_JOBS=$(TEST_BUILD_JOBS) cargo nextest run $(INTEGRATION_TEST_TARGETS) --test-threads $(TEST_RUN_THREADS)

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
