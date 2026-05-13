#!/usr/bin/env sh
set -eu

ENTRY="${1:-}"
[ -n "$ENTRY" ] || { echo "maybe-xrun.sh: missing entry argument" >&2; exit 1; }

# xrun blanks XRUN_LOCAL_MAKE when calling back — run local build.
[ "${XRUN_LOCAL_MAKE+set}" = "set" ] && exit 1

ACTIVE=no
[ -f .xrun-env ] && ACTIVE=$(awk -F= '/^active=/ {print $2}' .xrun-env 2>/dev/null)

if [ "$ACTIVE" = "yes" ] && [ -f xrun.conf ]; then
	command -v xrun >/dev/null 2>&1 || { echo "Missing xrun binary. Install it first." >&2; exit 1; }
	XRUN_CONFIG=xrun.conf xrun run "$ENTRY" || true
	# xrun handled it (or was interrupted) — do not fall through to local build.
	exit 0
fi

# xrun not active — run local build.
exit 1
