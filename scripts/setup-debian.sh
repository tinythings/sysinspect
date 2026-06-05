#!/usr/bin/env bash
# Debian/Ubuntu setup.
set -eu

run_setup() {
	local distro
	distro=$( . /etc/os-release 2>/dev/null && echo "$ID" )
	case "$distro" in
		debian|ubuntu) ;;
		*) echo "Unsupported Linux distribution: $distro" >&2; \
		   echo "Supported: Debian, Ubuntu, FreeBSD" >&2; exit 1 ;;
	esac

	for cmd in pkg-config clang protoc jq meson; do
		command -v "$cmd" >/dev/null 2>&1 && continue
		require_cmd apt-get
		echo "Installing Linux build packages with apt-get"
		$(sudo_cmd) apt-get update
		$(sudo_cmd) apt-get install -y pkg-config libssl-dev libffi-dev libsodium-dev libpam0g-dev llvm-dev libclang-dev clang protobuf-compiler jq meson
		break
	done
}
