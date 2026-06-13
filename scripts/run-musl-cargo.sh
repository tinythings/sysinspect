#!/usr/bin/env sh
set -eu

ROOT_DIR=$(pwd)
target="${1:-}"
cc="${2:-}"
shift 2

[ -n "$target" ] || { echo "run-musl-cargo.sh: missing target triple" >&2; exit 1; }
[ -n "$cc" ] || { echo "run-musl-cargo.sh: missing musl compiler" >&2; exit 1; }
[ "$#" -gt 0 ] || { echo "run-musl-cargo.sh: missing cargo arguments" >&2; exit 1; }

prefix="${ROOT_DIR}/target/musl/${target}"
libdir="${prefix}/lib"
includedir="${prefix}/include"

sh scripts/check-musl-target.sh "$target" "$cc" || status=$?
if [ "${status:-0}" -ne 0 ]; then
	if [ "$status" -eq 2 ]; then
		exit 0
	fi
	exit "$status"
fi

export LIBRARY_PATH="$libdir${LIBRARY_PATH:+:$LIBRARY_PATH}"
export C_INCLUDE_PATH="$includedir${C_INCLUDE_PATH:+:$C_INCLUDE_PATH}"
export CPLUS_INCLUDE_PATH="$includedir${CPLUS_INCLUDE_PATH:+:$CPLUS_INCLUDE_PATH}"
export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-Lnative=$libdir -C target-feature=+crt-static -C relocation-model=static -C link-arg=-static -C link-arg=-no-pie -C link-arg=-lc"

case " $* " in
	*" --release "*)
		# Keep the workspace's aggressive release profile for native builds, but
		# tone it down for musl until the release-startup crash is isolated.
		export CARGO_PROFILE_RELEASE_STRIP=none
		export CARGO_PROFILE_RELEASE_LTO=false
		;;
esac

exec cargo "$@"
