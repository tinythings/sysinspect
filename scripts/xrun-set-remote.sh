#!/usr/bin/env sh
set -eu

command -v xrun >/dev/null 2>&1 || { echo "Missing xrun binary. Install it first." >&2; exit 1; }
[ -f xrun.conf ] || { echo "No xrun.conf found in this project. Create one first." >&2; exit 1; }

printf 'active=yes\n' > .xrun-env
printf '\nRemote builds enabled. To switch back to local-only:\n    make set-local-builds\n\n'
