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

sh scripts/check-musl-target.sh "$target" "$cc"

export LIBRARY_PATH="$libdir${LIBRARY_PATH:+:$LIBRARY_PATH}"
export C_INCLUDE_PATH="$includedir${C_INCLUDE_PATH:+:$C_INCLUDE_PATH}"
export CPLUS_INCLUDE_PATH="$includedir${CPLUS_INCLUDE_PATH:+:$CPLUS_INCLUDE_PATH}"
export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-Lnative=$libdir"

exec cargo "$@"
