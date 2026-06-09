#!/usr/bin/env sh
set -eu

ROOT_DIR=$(pwd)
target="${1:-}"
cc="${2:-}"
prefix="${ROOT_DIR}/target/musl/${target}"
libdir="${prefix}/lib"
build_marker="${prefix}/.sysinspect-pam-pic"

warn_setup_first() {
	printf '\033[1;93m%s\033[0m\n' "$1" >&2
	printf '\033[1;93m%s\033[0m\n' "Run 'make setup' first." >&2
	exit 2
}

[ -n "$target" ] || { echo "check-musl-target.sh: missing target triple" >&2; exit 1; }
[ -n "$cc" ] || { echo "check-musl-target.sh: missing musl compiler" >&2; exit 1; }

command -v "$cc" >/dev/null 2>&1 || {
	warn_setup_first "Missing $cc for $target."
}

missing=no
for f in "$libdir/libpam.a" "$libdir/libpam_misc.a"; do
	if [ ! -f "$f" ]; then
		missing=yes
	fi
done

if [ ! -f "$build_marker" ]; then
	missing=yes
fi

if [ "$missing" = yes ]; then
	warn_setup_first "Missing musl PAM static libraries for $target in $libdir."
fi
