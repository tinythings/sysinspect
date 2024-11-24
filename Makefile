.DEFAULT_GOAL := build

ARC_VERSION := $(shell cat src/main.rs | grep 'static VERSION' | sed -e 's/.*=//g' -e 's/[" ;]//g')
ARC_NAME := sysinspect-${ARC_VERSION}

.PHONY:build

check:
	cargo clippy --all -- -Dwarnings -Aunused-variables -Adead-code

devel-musl:
	cargo build -v --workspace --target x86_64-unknown-linux-musl
	rm -rf target/x86_64-unknown-linux-musl/debug/sys/
	mkdir -p target/x86_64-unknown-linux-musl/debug/sys/
	mv target/x86_64-unknown-linux-musl/debug/proc target/x86_64-unknown-linux-musl/debug/sys/
	mv target/x86_64-unknown-linux-musl/debug/net target/x86_64-unknown-linux-musl/debug/sys/
	mv target/x86_64-unknown-linux-musl/debug/run target/x86_64-unknown-linux-musl/debug/sys/

musl:
	cargo build --release --workspace --target x86_64-unknown-linux-musl
	cargo build -p proc -p net -p run --release --target x86_64-unknown-linux-musl
	rm -rf target/x86_64-unknown-linux-musl/release/sys
	mkdir -p target/x86_64-unknown-linux-musl/release/sys
	mv target/x86_64-unknown-linux-musl/release/proc target/x86_64-unknown-linux-musl/release/sys/
	mv target/x86_64-unknown-linux-musl/release/net target/x86_64-unknown-linux-musl/release/sys/
	mv target/x86_64-unknown-linux-musl/release/run target/x86_64-unknown-linux-musl/release/sys/

devel:
	cargo build -v --workspace
	rm -rf target/debug/sys/
	mkdir -p target/debug/sys/
	mv target/debug/proc target/debug/sys/
	mv target/debug/net target/debug/sys/
	mv target/debug/run target/debug/sys/

build:
	cargo build --release --workspace
	cargo build -p proc -p net -p run --release
	rm -rf target/release/sys/
	mkdir -p target/release/sys/
	mv target/release/proc target/release/sys/
	mv target/release/net target/release/sys/
	mv target/release/run target/release/sys/

	# Move plugin binaries
	rm -rf target/release/sys
	mkdir -p target/release/sys
	mv target/release/proc target/release/sys/
	mv target/release/net target/release/sys/
	mv target/release/run target/release/sys/

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
