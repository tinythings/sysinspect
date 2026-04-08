#!/bin/sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
SMOKE_SUITE=run_sys
. "$ROOT_DIR/smoke-tests/lib.sh"

RUN_BIN=${1:-"$ROOT_DIR/target/debug/run"}

[ "$(uname -s)" = "FreeBSD" ] || {
    echo "FreeBSD only"
    exit 1
}

[ -x "$RUN_BIN" ] || {
    echo "Missing module binary: $RUN_BIN"
    exit 1
}

command -v perl >/dev/null 2>&1 || {
    echo "Missing perl"
    exit 1
}

json_get_retcode() {
    if command -v jq >/dev/null 2>&1; then
        printf '%s\n' "$1" | jq -r '.retcode'
    else
        printf '%s\n' "$1" | perl -ne 'print "$1\n" if /"retcode"\s*:\s*([0-9-]+)/'
    fi
}

json_has() {
    printf '%s\n' "$1" | grep -F "$2" >/dev/null
}

fail_now() {
    echo "FAIL: $1"
    return 1
}

run_echo_json() {
    printf '%s\n' '{"arguments":{"cmd":"printf hello"}}' | "$RUN_BIN"
}

run_stdin_json() {
    printf '%s\n' '{"arguments":{"cmd":"cat","send":"abc"}}' | "$RUN_BIN"
}

run_missing_cmd_json() {
    printf '%s\n' '{"arguments":{}}' | "$RUN_BIN"
}

run_env_json() {
    printf '%s\n' '{"arguments":{"cmd":"sh -c '\''printf %s \"$FOO\"'\''","env":"FOO=bar"}}' | "$RUN_BIN"
}

run_disown_json() {
    printf '%s\n' '{"options":["disown"],"arguments":{"cmd":"sleep 1"}}' | "$RUN_BIN"
}

test_echo_success() {
    OUTPUT=$(run_echo_json)
    [ "$(json_get_retcode "$OUTPUT")" = "0" ] || { fail_now "echo retcode bad"; return 1; }
    json_has "$OUTPUT" '"message":"module sys.run finished"' || { fail_now "echo message bad"; return 1; }
    json_has "$OUTPUT" '"stdout":"hello"' || { fail_now "echo stdout bad"; return 1; }
}

test_stdin_success() {
    OUTPUT=$(run_stdin_json)
    [ "$(json_get_retcode "$OUTPUT")" = "0" ] || { fail_now "stdin retcode bad"; return 1; }
    json_has "$OUTPUT" '"stdout":"abc"' || { fail_now "stdin stdout bad"; return 1; }
}

test_env_success() {
    OUTPUT=$(run_env_json)
    [ "$(json_get_retcode "$OUTPUT")" = "0" ] || { fail_now "env retcode bad"; return 1; }
    json_has "$OUTPUT" '"stdout":"bar"' || { fail_now "env stdout bad"; return 1; }
}

test_missing_cmd_fails() {
    OUTPUT=$(run_missing_cmd_json)
    [ "$(json_get_retcode "$OUTPUT")" = "1" ] || { fail_now "missing cmd should fail"; return 1; }
    json_has "$OUTPUT" '"message":"Missing command"' || { fail_now "missing cmd message bad"; return 1; }
}

test_disown_success() {
    OUTPUT=$(run_disown_json)
    [ "$(json_get_retcode "$OUTPUT")" = "0" ] || { fail_now "disown retcode bad"; return 1; }
    json_has "$OUTPUT" "is running in background" || { fail_now "disown message bad"; return 1; }
}

smoke_run run_sys.echo_success test_echo_success
smoke_run run_sys.stdin_success test_stdin_success
smoke_run run_sys.env_success test_env_success
smoke_run run_sys.missing_cmd_fails test_missing_cmd_fails
smoke_run run_sys.disown_success test_disown_success

smoke_finish
