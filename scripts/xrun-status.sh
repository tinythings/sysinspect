#!/usr/bin/env sh
set -eu

if [ -f .xrun-env ]; then
	ACTIVE=$(awk -F= '/^active=/ {print $2}' .xrun-env 2>/dev/null)
else
	ACTIVE=no
fi

if [ -f xrun.conf ]; then
	CONFIG_AVAILABLE=yes
else
	CONFIG_AVAILABLE=no
fi

if [ "$ACTIVE" = "yes" ] && [ "$CONFIG_AVAILABLE" = "yes" ]; then
	printf '\nxrun mode active — builds run across the target matrix.\n\n'
	printf '    To switch to local-only builds:\n'
	printf '    make set-local-builds\n\n'
elif [ "$CONFIG_AVAILABLE" = "yes" ]; then
	printf '\nxrun available but inactive.\n\n'
	printf '    To enable:\n'
	printf '    make set-remote-builds\n\n'
else
	printf '\nxrun not configured — no xrun.conf found.\n\n'
	printf '    Create xrun.conf (see xrun project docs), then:\n'
	printf '    make set-remote-builds\n\n'
fi
