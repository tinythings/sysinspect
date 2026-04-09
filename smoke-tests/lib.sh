#!/bin/sh

SMOKE_PASS=0
SMOKE_FAIL=0
SMOKE_SUITE=${SMOKE_SUITE:-smoke}
SMOKE_GREEN=$(printf '\033[92m')
SMOKE_RED=$(printf '\033[91m')
SMOKE_YELLOW=$(printf '\033[93m')
SMOKE_RESET=$(printf '\033[0m')

smoke_fail() {
    echo "FAIL: $1"
    return 1
}

smoke_run() {
    TEST_NAME=$1
    shift
    printf 'Running %s ... ' "$TEST_NAME"
    if "$@"; then
        SMOKE_PASS=$((SMOKE_PASS + 1))
        printf '%sOK%s\n' "$SMOKE_GREEN" "$SMOKE_RESET"
    else
        SMOKE_FAIL=$((SMOKE_FAIL + 1))
        printf '%sFAIL%s\n' "$SMOKE_RED" "$SMOKE_RESET"
    fi
}

smoke_finish() {
    echo
    printf '%sSmoke test %s summary: Ran %s tests, %s pass, %s fail%s\n' \
        "$SMOKE_YELLOW" \
        "$SMOKE_SUITE" \
        "$((SMOKE_PASS + SMOKE_FAIL))" \
        "$SMOKE_PASS" \
        "$SMOKE_FAIL" \
        "$SMOKE_RESET"
    echo
    [ -n "${SMOKE_SUMMARY_FILE:-}" ] && printf '%s %s\n' "$SMOKE_PASS" "$SMOKE_FAIL" >"$SMOKE_SUMMARY_FILE"
    [ "$SMOKE_FAIL" -eq 0 ]
}
