#!/usr/bin/env bash
set -eu

OS=$(uname -s)

# --- shared helpers (available to all per-OS scripts) ---
sudo_cmd() {
	if command -v sudo >/dev/null 2>&1; then echo sudo
	elif command -v doas >/dev/null 2>&1; then echo doas
	fi
}

require_cmd() {
	command -v "$1" >/dev/null 2>&1 || { echo "Missing $1. Install it first." >&2; exit 1; }
}

add_rustup_target() {
	rustup target list --installed | grep -qx "$1" || {
		echo "Adding target $1"
		rustup target add "$1"
	}
}

# --- dispatch to per-OS setup script ---
case "$OS" in
	Linux)   source scripts/setup-debian.sh ;;
	FreeBSD) source scripts/setup-freebsd.sh ;;

	*)
		echo "Unsupported setup host: $OS" >&2
		exit 1
		;;
esac

run_setup

# --- shared steps ---
cargo nextest --version >/dev/null 2>&1 || cargo install cargo-nextest --locked
command -v tokei >/dev/null 2>&1 || cargo install tokei --locked
cargo install mxrun || true

if [ "$OS" = "Linux" ]; then
	sh scripts/setup-musl-environment.sh
fi

add_rustup_target wasm32-wasip1
if [ "$OS" = "Linux" ]; then
	add_rustup_target aarch64-unknown-linux-musl
	add_rustup_target x86_64-unknown-linux-musl
fi

echo "Setup complete."
