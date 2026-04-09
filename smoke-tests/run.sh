#!/bin/sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
UNAME_S=$(uname -s)

PLATFORM_DIR=$(
    case "$UNAME_S" in
        FreeBSD) printf '%s\n' "$ROOT_DIR/smoke-tests/freebsd" ;;
        Linux) printf '%s\n' "$ROOT_DIR/smoke-tests/linux" ;;
        NetBSD) printf '%s\n' "$ROOT_DIR/smoke-tests/netbsd" ;;
        OpenBSD) printf '%s\n' "$ROOT_DIR/smoke-tests/openbsd" ;;
        *) printf '%s\n' "" ;;
    esac
)

[ -n "$PLATFORM_DIR" ] || {
    echo "Unsupported smoke-test platform: $UNAME_S"
    exit 1
}

[ -d "$PLATFORM_DIR" ] || {
    echo "No smoke-tests for $UNAME_S"
    exit 1
}

FOUND=0
PASS=0
FAIL=0
for test_script in "$PLATFORM_DIR"/*.sh; do
    [ -e "$test_script" ] || continue
    FOUND=1
    SUMMARY_FILE=/tmp/sysinspect-smoke.summary.$$
    rm -f "$SUMMARY_FILE"
    if SMOKE_SUMMARY_FILE="$SUMMARY_FILE" sh "$test_script" >/tmp/sysinspect-smoke.out.$$ 2>&1; then
        :
    else
        :
    fi
    cat /tmp/sysinspect-smoke.out.$$
    RESULT=$(cat "$SUMMARY_FILE" 2>/dev/null || true)
    if [ -n "$RESULT" ]; then
        PASS=$((PASS + $(printf '%s\n' "$RESULT" | awk '{print $1}')))
        FAIL=$((FAIL + $(printf '%s\n' "$RESULT" | awk '{print $2}')))
    else
        FAIL=$((FAIL + 1))
    fi
    rm -f /tmp/sysinspect-smoke.out.$$
    rm -f "$SUMMARY_FILE"
done

[ "$FOUND" -eq 1 ] || {
    echo "No smoke-tests in $PLATFORM_DIR"
    exit 1
}

echo
echo "Ran $((PASS + FAIL)) tests, $PASS pass, $FAIL fail"

[ "$FAIL" -eq 0 ]
