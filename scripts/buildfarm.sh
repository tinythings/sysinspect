#!/bin/sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CONFIG_PATH=${BUILDFARM_CONFIG:-}

die() {
    printf '%s\n' "$*" >&2
    exit 1
}

info() {
    printf '\033[1;92m%s\033[0m\n' "$*"
}

warn() {
    printf '\033[1;91m%s\033[0m\n' "$*" >&2
}

need_buildfarm_config() {
    [ -n "$CONFIG_PATH" ] || die "BUILDFARM_CONFIG is not set"
    [ -f "$CONFIG_PATH" ] || die "BUILDFARM_CONFIG does not exist: $CONFIG_PATH"
}

make_cmd_for_os() {
    case "$1" in
        FreeBSD) printf 'gmake' ;;
        *) printf 'make' ;;
    esac
}

parse_config() {
    need_buildfarm_config
    awk '
        /^[[:space:]]*$/ { next }
        /^[[:space:]]*#/ { next }
        NF == 1 && $1 == "local" {
            print "local\tlocal\tlocal"
            next
        }
        NF != 3 {
            printf "Invalid buildfarm line %d: expected 3 fields, got %d\n", NR, NF > "/dev/stderr"
            exit 1
        }
        index($3, ":") == 0 {
            printf "Invalid buildfarm line %d: missing host:/destination in third field\n", NR > "/dev/stderr"
            exit 1
        }
        { print $1 "\t" $2 "\t" $3 }
    ' "$CONFIG_PATH"
}

rsync_project() {
    rsync -az \
        --exclude .git \
        --exclude .github \
        --exclude .vscode \
        --exclude .idea \
        --exclude .buildfarm \
        --exclude target \
        --exclude build/stage \
        --exclude build/modules-dist \
        "$ROOT_DIR"/ "$1:$2/"
}

cmd_init() {
    parse_config | while IFS="$(printf '\t')" read -r os arch target; do
        if [ "$target" = "local" ]; then
            info "Project initialised at local"
            continue
        fi
        ssh_target=${target%%:*}
        destination=${target#*:}
        if ssh "$ssh_target" "rm -rf '$destination' && mkdir -p '$destination'"; then
            if rsync_project "$ssh_target" "$destination"; then
                info "Project initialised at $target"
            else
                warn "Project initialisation failed at $target: rsync failed"
                exit 1
            fi
        else
            warn "Project initialisation failed at $target: remote directory reset failed"
            exit 1
        fi
    done
}

case "${1:-}" in
    init) cmd_init ;;
    run) die "buildfarm run is handled by target/buildfarm/buildfarm now" ;;
    *)
        die "Usage: $0 init | run <entry>"
        ;;
esac
