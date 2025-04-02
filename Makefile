.DEFAULT_GOAL := build

ARC_VERSION := $(shell cat src/main.rs | grep 'static VERSION' | sed -e 's/.*=//g' -e 's/[" ;]//g')
ARC_NAME := sysinspect-${ARC_VERSION}

.PHONY:build

define deps
	@OS_ID=$$(lsb_release -si 2>/dev/null); \
	if [ "$$OS_ID" = "Ubuntu" ] || [ "$$OS_ID" = "Debian" ]; then \
		echo "Installing required packages: pkg-config, libssl-dev, libffi-dev"; \
		sudo apt-get update && sudo apt-get install -y pkg-config libssl-dev libffi-dev; \
	else \
		echo "Oops, no fun for $$OS_ID right now. Builds are only possible on Debian/Ubuntu." >&2; \
		echo "But! You can fix this by sending your PR here: https://github.com/tinythings/sysinspect :-)" >&2; \
		exit 1; \
	fi
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


define move_bin
	@dir=$$(if [ -n "$(2)" ]; then echo target/$(2)/$(1); else echo target/$(1); fi); \
	echo "Moving binaries in $$dir"; \
	rm -rf $$dir/sys; \
	mkdir -p $$dir/sys; \
	rm -rf $$dir/fs; \
	mkdir -p $$dir/fs; \
	mv $$dir/proc $$dir/sys/; \
	mv $$dir/net $$dir/sys/; \
	mv $$dir/run $$dir/sys/; \
	mv $$dir/ssrun $$dir/sys/; \
	mv $$dir/file $$dir/fs/;
endef

setup:
	$(call deps)
	$(call tgt,aarch64-unknown-linux-musl)
	$(call tgt,x86_64-unknown-linux-musl)

clean:
	cargo clean

check:
	cargo clippy --all -- -Dwarnings -Aunused-variables -Adead-code

fix:
	cargo clippy --fix --allow-dirty --allow-staged --all

musl-aarch64-dev:
	$(call check_present,aarch64-linux-musl-gcc)
	cargo build -v --workspace --target aarch64-unknown-linux-musl
	$(call move_bin,debug,aarch64-unknown-linux-musl)

musl-aarch64:
	$(call check_present,aarch64-linux-musl-gcc)
	cargo build --release --workspace --target aarch64-unknown-linux-musl
	$(call move_bin,release,aarch64-unknown-linux-musl)

musl-x86_64-dev:
	$(call check_present,x86_64-linux-musl-gcc)
	cargo build -v --workspace --target x86_64-unknown-linux-musl
	$(call move_bin,debug,x86_64-unknown-linux-musl)

musl-x86_64:
	$(call check_present,x86_64-linux-musl-gcc)
	cargo build --release --workspace --target x86_64-unknown-linux-musl
	$(call move_bin,release,x86_64-unknown-linux-musl)

devel:
	cargo build -v --workspace
	$(call move_bin,debug,)

build:
	cargo build --release --workspace
	$(call move_bin,release,)

man:
	pandoc --standalone --to man docs/manpages/sysinspect.8.md -o docs/manpages/sysinspect.8

test:
	cargo nextest run --workspace

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
