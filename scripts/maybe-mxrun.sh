#!/usr/bin/env sh
set -eu

ENTRY="${1:-}"
[ -n "$ENTRY" ] || { echo "maybe-mxrun.sh: missing entry argument" >&2; exit 1; }

# mxrun blanks MXRUN_LOCAL_MAKE when calling back — run local build.
[ "${MXRUN_LOCAL_MAKE+set}" = "set" ] && exit 1

ACTIVE=no
[ -f .mxrun-env ] && ACTIVE=$(awk -F= '/^active=/ {print $2}' .mxrun-env 2>/dev/null)

if [ "$ACTIVE" = "yes" ] && [ -f mxrun.conf ]; then
	command -v mxrun >/dev/null 2>&1 || { echo "Missing mxrun binary. Install it first." >&2; exit 1; }
	case "$ENTRY" in
		dev|all-dev|modules-dev|modules-dist-dev|modules-refresh-dev)
			LABEL="Development Build Mode" ;;
		test)
			LABEL="Testing Everything" ;;
		test-core)
			LABEL="Testing Core Libraries" ;;
		test-modules)
			LABEL="Testing Modules" ;;
		test-sensors)
			LABEL="Testing Sensors" ;;
		test-integration)
			LABEL="Integration Tests" ;;
		*)
			LABEL="Production Build Mode" ;;
	esac
	MXRUN_CONFIG=mxrun.conf mxrun run --label="$LABEL" "$ENTRY" || true
	# mxrun handled it (or was interrupted) — do not fall through to local build.
	exit 0
fi

# mxrun not active — run local build.
exit 1
