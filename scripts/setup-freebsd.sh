#!/usr/bin/env bash
# FreeBSD setup.
set -eu

run_setup() {
	for cmd in gmake cargo rustc pkgconf clang protoc jq; do
		command -v "$cmd" >/dev/null 2>&1 && continue
		require_cmd pkg
		echo "Installing FreeBSD build packages with pkg"
		$(sudo_cmd) pkg install -y gmake rust pkgconf llvm protobuf libffi libsodium openssl jq
		break
	done
}
