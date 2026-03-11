use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    process::{Command, Stdio},
    thread,
};

/// Run `net.http` binary with JSON request payload.
/// # Arguments
/// * `payload` - Module request payload
/// # Returns
/// * `Value` - Parsed JSON response
fn run_module(payload: &Value) -> Value {
    let bin = env!("CARGO_BIN_EXE_http");
    let mut child =
        Command::new(bin).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap_or_else(|err| panic!("failed to spawn net.http binary: {err}"));

    child
        .stdin
        .as_mut()
        .unwrap_or_else(|| panic!("net.http stdin is not available"))
        .write_all(payload.to_string().as_bytes())
        .unwrap_or_else(|err| panic!("failed to write module request payload: {err}"));

    let out = child.wait_with_output().unwrap_or_else(|err| panic!("failed to wait for net.http output: {err}"));
    assert!(out.status.success(), "net.http exited with status {}", out.status);

    serde_json::from_slice(&out.stdout).unwrap_or_else(|err| panic!("failed to parse net.http JSON output: {err}"))
}

/// Spawn a one-shot HTTP server and validate the inbound request.
/// # Arguments
/// * `assertion` - Request assertion callback
/// * `status` - Response status line suffix
/// * `headers` - Additional response headers
/// * `body` - Response body
/// # Returns
/// * `String` - Base URL of the server
fn spawn_server<F>(assertion: F, status: &str, headers: &[&str], body: &str) -> String
where
    F: Fn(&str) + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|err| panic!("failed to bind test server: {err}"));
    let addr = listener.local_addr().unwrap_or_else(|err| panic!("failed to read test server address: {err}"));
    let status = status.to_string();
    let headers = headers.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let body = body.to_string();

    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap_or_else(|err| panic!("failed to accept test connection: {err}"));
        let request = read_http_request(&mut stream);
        assertion(&request);

        let mut response = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n", body.len());
        for header in headers {
            response.push_str(&header);
            response.push_str("\r\n");
        }
        response.push_str("\r\n");
        response.push_str(&body);
        stream.write_all(response.as_bytes()).unwrap_or_else(|err| panic!("failed to write test response: {err}"));
    });

    format!("http://{addr}")
}

/// Read a raw HTTP request from the socket.
/// # Arguments
/// * `stream` - Accepted TCP stream
/// # Returns
/// * `String` - Raw HTTP request text
fn read_http_request(stream: &mut std::net::TcpStream) -> String {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut content_length = None;

    loop {
        let n = stream.read(&mut chunk).unwrap_or_else(|err| panic!("failed to read request: {err}"));
        if n == 0 {
            break;
        }

        buf.extend_from_slice(&chunk[..n]);
        if content_length.is_none()
            && let Some((head, _)) = split_http_request(&buf)
        {
            content_length = content_length_of(head);
        }

        if let Some((_, body)) = split_http_request(&buf)
            && body.len() >= content_length.unwrap_or(0)
        {
            break;
        }
    }

    String::from_utf8(buf).unwrap_or_else(|err| panic!("request is not UTF-8: {err}"))
}

/// Split a raw HTTP request into head and body bytes.
/// # Arguments
/// * `buf` - Raw request buffer
/// # Returns
/// * `Option<(&[u8], &[u8])>` - Header and body slices
fn split_http_request(buf: &[u8]) -> Option<(&[u8], &[u8])> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|idx| (&buf[..idx + 4], &buf[idx + 4..]))
}

/// Get content-length from a raw HTTP request head.
/// # Arguments
/// * `head` - Raw header bytes
/// # Returns
/// * `Option<usize>` - Parsed content-length
fn content_length_of(head: &[u8]) -> Option<usize> {
    String::from_utf8_lossy(head)
        .lines()
        .find_map(|line| line.strip_prefix("Content-Length: ").or_else(|| line.strip_prefix("content-length: ")))
        .and_then(|v| v.trim().parse::<usize>().ok())
}

/// Get a header value from a raw HTTP request.
/// # Arguments
/// * `request` - Raw HTTP request text
/// * `name` - Header name
/// # Returns
/// * `Option<&str>` - Header value
fn header_value<'a>(request: &'a str, name: &str) -> Option<&'a str> {
    request.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.eq_ignore_ascii_case(name)).then_some(value.trim())
    })
}

#[test]
fn test_http_module_gets_json_and_sends_query_and_header_auth() {
    let base = spawn_server(
        |request| {
            let lower = request.to_lowercase();
            assert!(request.starts_with("GET /issues?ticket=OPS-42 HTTP/1.1\r\n"));
            assert!(lower.contains("authorization: bearer secret-token\r\n"));
            assert!(lower.contains("accept: application/json\r\n"));
        },
        "200 OK",
        &["Content-Type: application/json"],
        r#"{"items":[1,2,3]}"#,
    );

    let out = run_module(&json!({
        "args": {
            "method": "GET",
            "url": format!("{base}/issues"),
            "query": { "ticket": "OPS-42" },
            "headers": { "Accept": "application/json" },
            "auth": { "type": "header", "header": "Authorization", "value": "Bearer secret-token" }
        }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(out.pointer("/data/status"), Some(&json!(200)));
    assert_eq!(out.pointer("/data/body/json/items"), Some(&json!([1, 2, 3])));
}

#[test]
fn test_http_module_gets_json_and_sends_bearer_auth() {
    let base = spawn_server(
        |request| {
            assert!(request.starts_with("GET /bearer HTTP/1.1\r\n"));
            assert_eq!(header_value(request, "Authorization"), Some("Bearer secret-token"));
        },
        "200 OK",
        &["Content-Type: application/json"],
        r#"{"ok":true}"#,
    );

    let out = run_module(&json!({
        "args": {
            "method": "GET",
            "url": format!("{base}/bearer"),
            "auth": { "type": "bearer", "token": "secret-token" }
        }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(out.pointer("/data/status"), Some(&json!(200)));
    assert_eq!(out.pointer("/data/body/json/ok"), Some(&json!(true)));
}

#[test]
fn test_http_module_posts_json_body_with_basic_auth() {
    let base = spawn_server(
        |request| {
            assert!(request.starts_with("POST /submit HTTP/1.1\r\n"));
            assert_eq!(header_value(request, "Authorization"), Some("Basic dXNlcjpzZWNyZXQ="));
            assert!(request.to_lowercase().contains("content-type: application/json\r\n"));
            assert!(request.contains(r#"{"ticket":"OPS-7","ok":true}"#));
        },
        "201 Created",
        &["Content-Type: application/json"],
        r#"{"accepted":true}"#,
    );

    let out = run_module(&json!({
        "args": {
            "method": "POST",
            "url": format!("{base}/submit"),
            "body": { "ticket": "OPS-7", "ok": true },
            "auth": { "type": "basic", "username": "user", "password": "secret" }
        }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(out.pointer("/data/status"), Some(&json!(201)));
    assert_eq!(out.pointer("/data/body/json/accepted"), Some(&json!(true)));
}

#[test]
fn test_http_module_sends_query_auth_parameter() {
    let base = spawn_server(
        |request| {
            assert!(request.starts_with("GET /query?access_token=secret-token HTTP/1.1\r\n"));
        },
        "200 OK",
        &["Content-Type: application/json"],
        r#"{"ok":true}"#,
    );

    let out = run_module(&json!({
        "args": {
            "method": "GET",
            "url": format!("{base}/query"),
            "auth": { "type": "query", "param": "access_token", "value": "secret-token" }
        }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(out.pointer("/data/status"), Some(&json!(200)));
    assert_eq!(out.pointer("/data/body/json/ok"), Some(&json!(true)));
}

#[test]
fn test_http_module_accepts_custom_ok_status() {
    let base = spawn_server(
        |request| {
            assert!(request.starts_with("GET /accepted HTTP/1.1\r\n"));
        },
        "202 Accepted",
        &["Content-Type: text/plain"],
        "queued",
    );

    let out = run_module(&json!({
        "args": {
            "method": "GET",
            "url": format!("{base}/accepted"),
            "ok-status": [202]
        }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(out.pointer("/data/status"), Some(&json!(202)));
    assert_eq!(out.pointer("/data/body/text"), Some(&json!("queued")));
}
