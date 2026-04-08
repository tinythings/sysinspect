#!/bin/sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
SMOKE_SUITE=net_sys
. "$ROOT_DIR/smoke-tests/lib.sh"

NET_BIN=${1:-"$ROOT_DIR/target/debug/net"}

[ "$(uname -s)" = "FreeBSD" ] || {
    echo "FreeBSD only"
    exit 1
}

[ -x "$NET_BIN" ] || {
    echo "Missing module binary: $NET_BIN"
    exit 1
}

command -v ifconfig >/dev/null 2>&1 || {
    echo "Missing ifconfig"
    exit 1
}

command -v netstat >/dev/null 2>&1 || {
    echo "Missing netstat"
    exit 1
}

command -v perl >/dev/null 2>&1 || {
    echo "Missing perl"
    exit 1
}

json_get_retcode() {
    printf '%s\n' "$1" | perl -ne 'print "$1\n" if /"retcode"\s*:\s*([0-9-]+)/'
}

json_has() {
    printf '%s\n' "$1" | grep -F "$2" >/dev/null
}

expected_ifaces() {
    ifconfig -a | awk '
function emit() {
    if (iface != "" && up == 1 && has_addr == 1) {
        print iface
    }
}
/^[^ \t]/ {
    emit()
    split($0, head, ":")
    iface=head[1]
    up=(index($0, "UP") > 0 ? 1 : 0)
    has_addr=0
    next
}
/^[ \t]+(inet|inet6|ether)[ \t]/ {
    if (up == 1) {
        has_addr=1
    }
    next
}
END {
    emit()
}
'
}

first_iface() {
    expected_ifaces | head -n 1
}

second_iface() {
    expected_ifaces | sed -n '2p'
}

default_gateway() {
    netstat -rn -f inet | awk '$1 == "default" { print $2; exit }'
}

if_up_json() {
    printf '%s\n' '{"options":["if-up"]}' | "$NET_BIN"
}

route_json() {
    printf '%s\n' '{"options":["route-table"]}' | "$NET_BIN"
}

filtered_if_json() {
    printf '%s\n' "{\"options\":[\"if-up\"],\"arguments\":{\"if-list\":\"$1\"}}" | "$NET_BIN"
}

filtered_both_json() {
    printf '%s\n' "{\"options\":[\"if-up\",\"route-table\"],\"arguments\":{\"if-list\":\"$1\"}}" | "$NET_BIN"
}

missing_if_json() {
    printf '%s\n' '{"options":["if-up"],"arguments":{"if-list":"__sysinspect_missing_iface__"}}' | "$NET_BIN"
}

test_if_up_success() {
    OUTPUT=$(if_up_json)
    [ "$(json_get_retcode "$OUTPUT")" = "0" ] || smoke_fail "if-up retcode bad"
    json_has "$OUTPUT" '"message":"Network data obtained"' || smoke_fail "if-up message bad"
    json_has "$OUTPUT" '"if-up":{' || smoke_fail "if-up payload missing"
}

test_if_up_matches_system() {
    OUTPUT=$(if_up_json)
    for iface in $(expected_ifaces); do
        json_has "$OUTPUT" "\"$iface\":" || smoke_fail "missing interface $iface"
    done
}

test_if_up_filter_exact() {
    IFACE=$(first_iface)
    [ -n "$IFACE" ] || smoke_fail "no interface for filter test"
    OUTPUT=$(filtered_if_json "$IFACE")
    [ "$(json_get_retcode "$OUTPUT")" = "0" ] || smoke_fail "filtered if-up retcode bad"
    json_has "$OUTPUT" "\"$IFACE\":" || smoke_fail "filtered interface missing"
    OTHER=$(second_iface)
    [ -z "$OTHER" ] || ! json_has "$OUTPUT" "\"$OTHER\":" || smoke_fail "unexpected extra interface $OTHER"
}

test_missing_iface_fails() {
    OUTPUT=$(missing_if_json)
    [ "$(json_get_retcode "$OUTPUT")" = "1" ] || smoke_fail "missing iface should fail"
    json_has "$OUTPUT" 'missing network interfaces: __sysinspect_missing_iface__' || smoke_fail "missing iface message bad"
}

test_route_table_success() {
    OUTPUT=$(route_json)
    [ "$(json_get_retcode "$OUTPUT")" = "0" ] || smoke_fail "route-table retcode bad"
    json_has "$OUTPUT" '"message":"Network data obtained"' || smoke_fail "route-table message bad"
}

test_route_table_default_gateway() {
    GATEWAY=$(default_gateway)
    [ -z "$GATEWAY" ] && return 0
    OUTPUT=$(route_json)
    json_has "$OUTPUT" "\"gateway\":\"$GATEWAY\"" || smoke_fail "default gateway missing"
    json_has "$OUTPUT" '"mask":"0"' || smoke_fail "default route mask missing"
}

test_combined_if_up_and_route_table() {
    IFACE=$(first_iface)
    [ -n "$IFACE" ] || smoke_fail "no interface for combined test"
    OUTPUT=$(filtered_both_json "$IFACE")
    [ "$(json_get_retcode "$OUTPUT")" = "0" ] || smoke_fail "combined retcode bad"
    json_has "$OUTPUT" "\"$IFACE\":" || smoke_fail "combined if-up missing"
    json_has "$OUTPUT" '"route-table":[' || smoke_fail "combined route-table missing"
}

smoke_run net_sys.if_up_success test_if_up_success
smoke_run net_sys.if_up_matches_system test_if_up_matches_system
smoke_run net_sys.if_up_filter_exact test_if_up_filter_exact
smoke_run net_sys.missing_iface_fails test_missing_iface_fails
smoke_run net_sys.route_table_success test_route_table_success
smoke_run net_sys.route_table_default_gateway test_route_table_default_gateway
smoke_run net_sys.combined_if_up_and_route_table test_combined_if_up_and_route_table

smoke_finish
