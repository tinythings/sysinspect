#!/usr/bin/env sh
set -eu

if [ -f .mxrun-env ]; then
	ACTIVE=$(awk -F= '/^active=/ {print $2}' .mxrun-env 2>/dev/null)
else
	ACTIVE=no
fi

if [ -f mxrun.conf ]; then
	CONFIG_AVAILABLE=yes
else
	CONFIG_AVAILABLE=no
fi

if [ "$ACTIVE" = "yes" ] && [ "$CONFIG_AVAILABLE" = "yes" ]; then
	printf '\nmxrun mode active — builds run across the target matrix.\n\n'
	printf '    To switch to local-only builds:\n'
	printf '    make set-local-builds\n\n'
elif [ "$CONFIG_AVAILABLE" = "yes" ]; then
	printf '\nmxrun available but inactive.\n\n'
	printf '    To enable:\n'
	printf '    make set-remote-builds\n\n'
else
	printf '\nmxrun not configured — no mxrun.conf found.\n\n'
	printf '    Create mxrun.conf (see mxrun project docs), then:\n'
	printf '    make set-remote-builds\n\n'
fi
