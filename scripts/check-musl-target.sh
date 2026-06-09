#!/usr/bin/env sh
set -eu

ROOT_DIR=$(pwd)
target="${1:-}"
cc="${2:-}"
prefix="${ROOT_DIR}/target/musl/${target}"
libdir="${prefix}/lib"

[ -n "$target" ] || { echo "check-musl-target.sh: missing target triple" >&2; exit 1; }
[ -n "$cc" ] || { echo "check-musl-target.sh: missing musl compiler" >&2; exit 1; }

command -v "$cc" >/dev/null 2>&1 || {
	echo "Missing $cc for $target." >&2
	echo "Run 'make setup' first." >&2
	exit 1
}

missing=no
for f in "$libdir/libpam.a" "$libdir/libpam_misc.a"; do
	if [ ! -f "$f" ]; then
		missing=yes
	fi
done

if [ "$missing" = yes ]; then
	echo "Missing musl PAM static libraries for $target in $libdir." >&2
	echo "Run 'make setup' first." >&2
	exit 1
fi
