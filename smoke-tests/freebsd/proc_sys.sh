#!/bin/sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
SMOKE_SUITE=proc_sys
. "$ROOT_DIR/smoke-tests/lib.sh"

PROC_BIN=${1:-"$ROOT_DIR/target/debug/proc"}
PROC_WAIT_SECS=3

[ "$(uname -s)" = "FreeBSD" ] || {
    echo "FreeBSD only"
    exit 1
}

[ -x "$PROC_BIN" ] || {
    echo "Missing module binary: $PROC_BIN"
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

json_get_pid() {
    if command -v jq >/dev/null 2>&1; then
        printf '%s\n' "$1" | jq -r '.data.pid // empty'
    else
        printf '%s\n' "$1" | perl -ne 'print "$1\n" if /"pid"\s*:\s*([0-9]+)/'
    fi
}

json_has() {
    printf '%s\n' "$1" | grep -F "$2" >/dev/null
}

fail_now() {
    echo "FAIL: $1"
    return 1
}

spawn_probe_process() {
    sleep "$PROC_WAIT_SECS" &
    CHILD_PID=$!
}

probe_search() {
    printf '%s\n' 'sleep'
}

proc_pid_json() {
    printf '%s\n' "{\"options\":[\"pid\"],\"arguments\":{\"search\":\"$(probe_search)\"}}" | "$PROC_BIN"
}

proc_limits_json() {
    printf '%s\n' "{\"options\":[\"pid\",\"limits\"],\"arguments\":{\"search\":\"$(probe_search)\"}}" | "$PROC_BIN"
}

proc_missing_json() {
    printf '%s\n' '{"options":["pid"],"arguments":{"search":"__sysinspect_missing_process__"}}' | "$PROC_BIN"
}

proc_empty_search_json() {
    printf '%s\n' '{"options":["pid"]}' | "$PROC_BIN"
}

CHILD_PID=
spawn_probe_process
trap 'kill "$CHILD_PID" >/dev/null 2>&1 || true; wait "$CHILD_PID" 2>/dev/null || true' EXIT INT TERM
echo "Running proc_sys setup ... wait up to ${PROC_WAIT_SECS}s for probe process"
sleep 1

test_pid_success() {
    OUTPUT=$(proc_pid_json)
    [ "$(json_get_retcode "$OUTPUT")" = "0" ] || { fail_now "pid retcode bad"; return 1; }
    [ "$(json_get_pid "$OUTPUT")" = "$CHILD_PID" ] || { fail_now "pid mismatch"; return 1; }
    json_has "$OUTPUT" '"message":"Process is running"' || { fail_now "pid message bad"; return 1; }
}

test_limits_success() {
    OUTPUT=$(proc_limits_json)
    [ "$(json_get_retcode "$OUTPUT")" = "0" ] || { fail_now "limits retcode bad"; return 1; }
    [ "$(json_get_pid "$OUTPUT")" = "$CHILD_PID" ] || { fail_now "limits pid mismatch"; return 1; }
    json_has "$OUTPUT" '"limits":{' || { fail_now "limits payload missing"; return 1; }
}

test_missing_process_fails() {
    OUTPUT=$(proc_missing_json)
    [ "$(json_get_retcode "$OUTPUT")" = "1" ] || { fail_now "missing process should fail"; return 1; }
    json_has "$OUTPUT" '"message":"Process not found"' || { fail_now "missing process message bad"; return 1; }
}

test_empty_search_fails() {
    OUTPUT=$(proc_empty_search_json)
    [ "$(json_get_retcode "$OUTPUT")" = "1" ] || { fail_now "empty search should fail"; return 1; }
    json_has "$OUTPUT" '"message":"Search criteria is not defined"' || { fail_now "empty search message bad"; return 1; }
}

smoke_run proc_sys.pid_success test_pid_success
smoke_run proc_sys.limits_success test_limits_success
smoke_run proc_sys.missing_process_fails test_missing_process_fails
smoke_run proc_sys.empty_search_fails test_empty_search_fails

smoke_finish
