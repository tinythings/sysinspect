#!/usr/bin/env bash
# Debian/Ubuntu setup.
set -eu

run_setup() {
	for cmd in pkg-config clang protoc jq meson; do
		command -v "$cmd" >/dev/null 2>&1 && continue
		require_cmd apt-get
		echo "Installing Linux build packages with apt-get"
		$(sudo_cmd) apt-get update
		$(sudo_cmd) apt-get install -y pkg-config libssl-dev libffi-dev libsodium-dev libpam0g-dev llvm-dev libclang-dev clang protobuf-compiler jq meson
		break
	done
}
