#!/bin/sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
SMOKE_SUITE=http_net
. "$ROOT_DIR/smoke-tests/lib.sh"

HTTP_BIN=${1:-"$ROOT_DIR/target/debug/http"}
HTTP_PORT=18080

[ "$(uname -s)" = "FreeBSD" ] || {
    echo "FreeBSD only"
    exit 1
}

[ -x "$HTTP_BIN" ] || {
    echo "Missing module binary: $HTTP_BIN"
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

start_server() {
    perl -MIO::Socket::INET -e '
        my $server = IO::Socket::INET->new(
            LocalAddr => "127.0.0.1",
            LocalPort => $ARGV[0],
            Proto => "tcp",
            Listen => 5,
            ReuseAddr => 1,
        ) or die $!;
        my $count = 0;
        while ($count < 4) {
            my $client = $server->accept() or next;
            my $request = "";
            while (my $line = <$client>) {
                $request .= $line;
                last if $line eq "\r\n";
            }
            if ($request =~ m{GET /json}) {
                print $client "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: 12\r\n\r\n{\"ok\":true}\n";
            } elsif ($request =~ m{POST /echo}) {
                my ($len) = $request =~ /Content-Length:\s*(\d+)/i;
                my $body = "";
                read($client, $body, $len || 0);
                my $payload = "{\"echo\":" . $body . "}";
                print $client "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: " . length($payload) . "\r\n\r\n$payload";
            } elsif ($request =~ m{GET /missing}) {
                print $client "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nConnection: close\r\nContent-Length: 9\r\n\r\nnot found";
            } else {
                print $client "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nConnection: close\r\nContent-Length: 2\r\n\r\nok";
            }
            close $client;
            $count++;
        }
    ' "$HTTP_PORT" &
    SERVER_PID=$!
}

http_json_get() {
    printf '%s\n' "{\"arguments\":{\"method\":\"GET\",\"url\":\"http://127.0.0.1:${HTTP_PORT}/json\"}}" | "$HTTP_BIN"
}

http_post_echo() {
    printf '%s\n' "{\"arguments\":{\"method\":\"POST\",\"url\":\"http://127.0.0.1:${HTTP_PORT}/echo\",\"body\":{\"x\":1}}}" | "$HTTP_BIN"
}

http_404_fail() {
    printf '%s\n' "{\"arguments\":{\"method\":\"GET\",\"url\":\"http://127.0.0.1:${HTTP_PORT}/missing\"}}" | "$HTTP_BIN"
}

http_404_ok_status() {
    printf '%s\n' "{\"arguments\":{\"method\":\"GET\",\"url\":\"http://127.0.0.1:${HTTP_PORT}/missing\",\"ok-status\":[404]}}" | "$HTTP_BIN"
}

test_get_json_success() {
    OUTPUT=$(http_json_get)
    [ "$(json_get_retcode "$OUTPUT")" = "0" ] || { fail_now "get retcode bad"; return 1; }
    json_has "$OUTPUT" '"message":"HTTP request completed"' || { fail_now "get message bad"; return 1; }
    json_has "$OUTPUT" '"status":200' || { fail_now "get status bad"; return 1; }
    json_has "$OUTPUT" '"ok":true' || { fail_now "get ok bad"; return 1; }
    json_has "$OUTPUT" '"json":{"ok":true}' || { fail_now "get body json bad"; return 1; }
}

test_post_echo_success() {
    OUTPUT=$(http_post_echo)
    [ "$(json_get_retcode "$OUTPUT")" = "0" ] || { fail_now "post retcode bad"; return 1; }
    json_has "$OUTPUT" '"status":200' || { fail_now "post status bad"; return 1; }
    json_has "$OUTPUT" '"json":{"echo":{"x":1}}' || { fail_now "post echo body bad"; return 1; }
}

test_404_fails() {
    OUTPUT=$(http_404_fail)
    [ "$(json_get_retcode "$OUTPUT")" = "1" ] || { fail_now "404 should fail"; return 1; }
    json_has "$OUTPUT" '"status":404' || { fail_now "404 status bad"; return 1; }
    json_has "$OUTPUT" '"ok":false' || { fail_now "404 ok bad"; return 1; }
}

test_404_ok_status_passes() {
    OUTPUT=$(http_404_ok_status)
    [ "$(json_get_retcode "$OUTPUT")" = "0" ] || { fail_now "404 ok-status retcode bad"; return 1; }
    json_has "$OUTPUT" '"status":404' || { fail_now "404 ok-status status bad"; return 1; }
    json_has "$OUTPUT" '"ok":true' || { fail_now "404 ok-status ok bad"; return 1; }
}

SERVER_PID=
start_server
trap 'kill "$SERVER_PID" >/dev/null 2>&1 || true; wait "$SERVER_PID" 2>/dev/null || true' EXIT INT TERM
sleep 1

smoke_run http_net.get_json_success test_get_json_success
smoke_run http_net.post_echo_success test_post_echo_success
smoke_run http_net.404_fails test_404_fails
smoke_run http_net.404_ok_status_passes test_404_ok_status_passes

smoke_finish
