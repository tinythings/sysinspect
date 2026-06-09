#!/usr/bin/env bash
# Fedora setup.
set -eu

run_setup() {
	need_dnf=false

	if ! command -v pkg-config >/dev/null 2>&1; then need_dnf=true; fi
	if ! command -v clang >/dev/null 2>&1; then need_dnf=true; fi
	if ! command -v protoc >/dev/null 2>&1; then need_dnf=true; fi
	if ! command -v jq >/dev/null 2>&1; then need_dnf=true; fi
	if ! command -v meson >/dev/null 2>&1; then need_dnf=true; fi
	if ! command -v x86_64-linux-musl-gcc >/dev/null 2>&1; then need_dnf=true; fi
	if ! command -v rustup-init >/dev/null 2>&1; then need_dnf=true; fi
	if ! perl -e 'use File::Spec' 2>/dev/null; then need_dnf=true; fi

	if $need_dnf; then
		require_cmd dnf
		echo "Installing Fedora build packages with dnf"
		$(sudo_cmd) dnf install -y rustup perl pkgconf clang clang-devel protobuf-compiler jq meson musl-gcc musl-libc-static \
			openssl-devel libffi-devel libsodium-devel pam-devel llvm-devel
		hash -r
	fi

	ensure_helper_link x86_64-linux-gnu-gcc gcc
	ensure_helper_link x86_64-linux-musl-gcc musl-gcc

	if ! command -v rustup >/dev/null 2>&1; then
		if command -v rustup-init >/dev/null 2>&1; then
			echo "Running rustup-init"
			toolchain=$(awk -F\" '/channel/ {print $2}' rust-toolchain.toml 2>/dev/null || echo stable)
			rustup-init -y --default-toolchain "$toolchain"
		fi
		[ -f "$HOME/.cargo/env" ] || { echo "rustup-init failed." >&2; exit 1; }
	fi

	. "$HOME/.cargo/env"

	if ! rustc -vV >/dev/null 2>&1; then
		toolchain=$(awk -F\" '/channel/ {print $2}' rust-toolchain.toml 2>/dev/null || echo stable)
		echo "Reinstalling toolchain $toolchain"
		rustup toolchain remove "$toolchain" 2>/dev/null || true
		rustup toolchain install "$toolchain"
		rustup default "$toolchain"
	fi
}
