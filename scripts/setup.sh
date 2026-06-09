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

ensure_helper_link() {
	want="$1"
	have="$2"
	if command -v "$want" >/dev/null 2>&1; then
		return
	fi
	if ! command -v "$have" >/dev/null 2>&1; then
		return
	fi
	echo "Creating $want symlink"
	mkdir -p "$HOME/.cargo/bin"
	ln -sf "$(command -v "$have")" "$HOME/.cargo/bin/$want"
}

add_rustup_target() {
	rustup target list --installed | grep -qx "$1" || {
		echo "Adding target $1"
		rustup target add "$1"
	}
}

# --- dispatch to per-OS setup script ---
case "$OS" in
		Linux)
			distro=$( . /etc/os-release 2>/dev/null && echo "$ID" )
			case "$distro" in
				debian|ubuntu) . scripts/setup-debian.sh ;;
				fedora)        . scripts/setup-fedora.sh ;;
				*)
					echo "Unsupported Linux distribution: $distro" >&2
					echo "Supported: Debian, Ubuntu, Fedora, FreeBSD" >&2
					exit 1
					;;
			esac
			;;
		FreeBSD) . scripts/setup-freebsd.sh ;;
		*)
			echo "Unsupported setup host: $OS" >&2
			exit 1
		;;
esac

run_setup

# --- shared steps ---
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
require_cmd cargo
command -v rustup >/dev/null 2>&1 || { echo "Missing rustup. Install it first." >&2; exit 1; }

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
