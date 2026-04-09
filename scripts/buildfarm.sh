#!/bin/sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CONFIG_PATH=${BUILDFARM_CONFIG:-}
LOG_ROOT="$ROOT_DIR/.buildfarm/logs"
SESSION_PREFIX="sysinspect-buildfarm"
LOCAL_MAKE_CMD=${BUILDFARM_LOCAL_MAKE:-make}

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

cmd_run() {
    entry=${1:-}
    [ -n "$entry" ] || die "Missing buildfarm entry"
    need_buildfarm_config
    command -v tmux >/dev/null 2>&1 || die "tmux is required for buildfarm runs"

    session_name="$SESSION_PREFIX-$entry"
    run_root="$LOG_ROOT/$entry"
    targets_file=$(mktemp)
    trap 'rm -f "$targets_file"' EXIT INT TERM
    parse_config > "$targets_file"
    [ -s "$targets_file" ] || die "Buildfarm config has no targets"

    rm -rf "$run_root"
    mkdir -p "$run_root"
    tmux has-session -t "$session_name" >/dev/null 2>&1 && tmux kill-session -t "$session_name"
    tmux new-session -d -s "$session_name" -n "$entry"
    tmux set-option -t "$session_name" remain-on-exit on >/dev/null

    pane_index=0
    while IFS="$(printf '\t')" read -r os arch target; do
        log_file="$run_root/$(printf '%s' "$target" | tr '/:@' '___').log"
        status_file="$run_root/$(printf '%s' "$target" | tr '/:@' '___').status"
        signal_name="$session_name-$pane_index"
        make_cmd=$(make_cmd_for_os "$os")
        if [ "$target" = "local" ]; then
            pane_cmd=$(cat <<EOF
rm -f '$log_file' '$status_file'
(
  printf '==> %s %s %s\\n' 'local' 'local' 'local'
  cd '$ROOT_DIR'
  BUILDFARM_CONFIG= BUILDFARM_LOCAL_MAKE= $LOCAL_MAKE_CMD $entry
  printf '%s' "\$?" > '$status_file'
) 2>&1 | tee '$log_file'
rc=\$(cat '$status_file' 2>/dev/null || printf '1')
if [ "\$rc" = 0 ]; then
  printf '\\033[1;92m%s finished\\033[0m\\n' '$entry'
else
  printf '\\033[1;91m%s failed\\033[0m\\n' '$entry'
fi
tmux wait-for -S '$signal_name'
EOF
)
        else
            ssh_target=${target%%:*}
            destination=${target#*:}
            pane_cmd=$(cat <<EOF
rm -f '$log_file' '$status_file'
(
  printf '==> %s %s %s\\n' '$os' '$arch' '$target'
  if rsync -az --exclude .git --exclude .github --exclude .vscode --exclude .idea --exclude .buildfarm --exclude target --exclude build/stage --exclude build/modules-dist '$ROOT_DIR'/ '$ssh_target:$destination/'; then
    ssh -tt '$ssh_target' "cd '$destination' && $make_cmd $entry"
    printf '%s' "\$?" > '$status_file'
  else
    printf '%s' '111' > '$status_file'
  fi
) 2>&1 | tee '$log_file'
rc=\$(cat '$status_file' 2>/dev/null || printf '1')
if [ "\$rc" = 0 ]; then
  printf '\\033[1;92m%s finished\\033[0m\\n' '$entry'
else
  printf '\\033[1;91m%s failed\\033[0m\\n' '$entry'
fi
tmux wait-for -S '$signal_name'
EOF
)
        fi

        if [ "$pane_index" -eq 0 ]; then
            tmux send-keys -t "$session_name:0.0" "$pane_cmd" C-m
        else
            tmux split-window -t "$session_name:0" -v
            tmux select-layout -t "$session_name:0" tiled >/dev/null
            tmux send-keys -t "$session_name:0.$pane_index" "$pane_cmd" C-m
        fi
        pane_index=$((pane_index + 1))
    done < "$targets_file"

    wait_script="$run_root/waiter.sh"
    {
        printf '#!/bin/sh\nset -eu\n'
        i=0
        while [ "$i" -lt "$pane_index" ]; do
            printf "tmux wait-for '%s-%s'\n" "$session_name" "$i"
            i=$((i + 1))
        done
        printf "if tmux display-popup -E \"printf '\\\\033[1;92mBuild finished across the farm\\\\033[0m\\\\n'; printf 'Press Enter to close...'; read _\" >/dev/null 2>&1; then :; else tmux display-message '\\\\033[1;92mBuild finished across the farm\\\\033[0m'; fi\n"
    } > "$wait_script"
    chmod +x "$wait_script"
    tmux new-window -d -t "$session_name" -n waiter "sh '$wait_script'"
    tmux select-window -t "$session_name:0"
    tmux attach-session -t "$session_name"
}

case "${1:-}" in
    init) cmd_init ;;
    run) shift; cmd_run "${1:-}" ;;
    *)
        die "Usage: $0 init | run <entry>"
        ;;
esac
