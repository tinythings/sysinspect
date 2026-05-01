#!/usr/bin/env sh
set -eu
printf 'active=no\n' > .xrun-env
printf '\nLocal builds enabled. To re-enable xrun:\n    make set-remote-builds\n\n'
