#!/usr/bin/env sh
set -eu
printf 'active=no\n' > .mxrun-env
printf '\nLocal builds enabled. To re-enable mxrun:\n    make set-remote-builds\n\n'
