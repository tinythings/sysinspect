#!/usr/bin/env bash
# Debian/Ubuntu setup.
set -eu

run_setup() {
	for cmd in pkg-config clang protoc jq meson wget tar curl x86_64-linux-musl-gcc; do
		command -v "$cmd" >/dev/null 2>&1 && continue
		require_cmd apt-get
		echo "Installing Linux build packages with apt-get"
		$(sudo_cmd) apt-get update
		$(sudo_cmd) apt-get install -y build-essential pkg-config musl-tools wget curl ca-certificates xz-utils libssl-dev libffi-dev libsodium-dev libpam0g-dev llvm-dev libclang-dev clang protobuf-compiler jq meson
		break
	done

	if ! command -v rustup >/dev/null 2>&1 || ! command -v cargo >/dev/null 2>&1; then
		echo "Installing rustup and Cargo toolchain"
		toolchain=$(awk -F\" '/channel/ {print $2}' rust-toolchain.toml 2>/dev/null || echo stable)
		curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain "$toolchain"
	fi

	[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
	if command -v rustup >/dev/null 2>&1 && ! command -v cargo >/dev/null 2>&1; then
		echo "Activating Rust toolchain"
		toolchain=$(awk -F\" '/channel/ {print $2}' rust-toolchain.toml 2>/dev/null || echo stable)
		rustup toolchain install "$toolchain"
		rustup default "$toolchain"
		[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
	fi
}
