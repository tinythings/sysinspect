use crate::{MeNotifyContext, MeNotifyError, MeNotifyEventBuilder, MeNotifyHost, MeNotifyProgram, MeNotifyRunner, MeNotifyRuntime};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

fn spawn_http_server(response: &'static str, seen: Arc<Mutex<String>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener.local_addr().expect("addr should resolve");

    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("server should accept");
        let mut buf = [0u8; 8192];
        let n = stream.read(&mut buf).expect("server should read");
        *seen.lock().expect("lock should work") = String::from_utf8_lossy(&buf[..n]).to_string();
        stream.write_all(response.as_bytes()).expect("server should write");
        stream.flush().expect("server should flush");
    });

    format!("http://{addr}")
}

#[test]
fn tick_runner_exposes_http_get() {
    let seen = Arc::new(Mutex::new(String::new()));
    let url = spawn_http_server("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 17\r\n\r\n{\"hello\":\"world\"}", seen.clone());

    let tmp = tempfile::tempdir().expect("tempdir should be created");
    let root = tmp.path().join("lib/sensors/lua");
    fs::create_dir_all(&root).expect("script root should be created");
    fs::write(
        root.join("demo.lua"),
        r#"
return {
  tick = function(ctx)
    local rsp = http.get(ctx.args.url, { headers = { ["X-Test"] = "abc" } })
    ctx.emit({ status = rsp.status, ok = rsp.ok, hello = rsp.json.hello, ctype = rsp.headers["content-type"] })
  end
}
"#,
    )
    .expect("script file should be written");

    let runtime = MeNotifyRuntime::with_sharelib_root("demo".to_string(), "menotify.demo".to_string(), tmp.path().to_path_buf());
    let runner = MeNotifyRunner::new(
        MeNotifyProgram::new(&runtime).expect("program should load"),
        MeNotifyContext::new(
            "demo",
            "menotify.demo",
            "demo",
            &[],
            &serde_yaml::from_str(&format!("url: {url}\n")).expect("yaml should parse"),
            Some(Duration::from_secs(1)),
        ),
    );
    let out = Mutex::new(Vec::new());

    runner
        .run_tick_with_emit(&|ev| out.lock().expect("lock should work").push(ev), &MeNotifyEventBuilder::new("demo", "menotify.demo", None))
        .expect("tick with http get should succeed");

    let events = out.lock().expect("lock should work");
    assert_eq!(events[0]["data"]["status"], 200);
    assert_eq!(events[0]["data"]["hello"], "world");
    assert_eq!(events[0]["data"]["ctype"], "application/json");
    assert!(events[0]["data"]["ok"].as_bool().expect("ok should be bool"));
    assert!(seen.lock().expect("lock should work").to_lowercase().contains("x-test: abc"));
}

#[test]
fn tick_runner_exposes_http_request() {
    let seen = Arc::new(Mutex::new(String::new()));
    let url = spawn_http_server("HTTP/1.1 201 Created\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nok", seen.clone());

    let tmp = tempfile::tempdir().expect("tempdir should be created");
    let root = tmp.path().join("lib/sensors/lua");
    fs::create_dir_all(&root).expect("script root should be created");
    fs::write(
        root.join("demo.lua"),
        r#"
return {
  tick = function(ctx)
    local rsp = http.request({
      url = ctx.args.url,
      method = "POST",
      headers = { ["Content-Type"] = "text/plain" },
      body = "ping",
      parse_json = false
    })
    ctx.emit({ status = rsp.status, body = rsp.body })
  end
}
"#,
    )
    .expect("script file should be written");

    let runtime = MeNotifyRuntime::with_sharelib_root("demo".to_string(), "menotify.demo".to_string(), tmp.path().to_path_buf());
    let runner = MeNotifyRunner::new(
        MeNotifyProgram::new(&runtime).expect("program should load"),
        MeNotifyContext::new(
            "demo",
            "menotify.demo",
            "demo",
            &[],
            &serde_yaml::from_str(&format!("url: {url}\n")).expect("yaml should parse"),
            Some(Duration::from_secs(1)),
        ),
    );
    let out = Mutex::new(Vec::new());

    runner
        .run_tick_with_emit(&|ev| out.lock().expect("lock should work").push(ev), &MeNotifyEventBuilder::new("demo", "menotify.demo", None))
        .expect("tick with http request should succeed");

    let events = out.lock().expect("lock should work");
    assert_eq!(events[0]["data"]["status"], 201);
    assert_eq!(events[0]["data"]["body"], "ok");

    let req = seen.lock().expect("lock should work").to_lowercase();
    assert!(req.starts_with("post "));
    assert!(req.contains("content-type: text/plain"));
    assert!(req.ends_with("ping"));
}

#[test]
fn http_request_rejects_nan_timeout() {
    let err = MeNotifyHost::timeout_for_test(f64::NAN).expect_err("nan timeout should fail");

    assert!(matches!(err, MeNotifyError::HttpSpec(_)));
}

#[test]
fn http_request_rejects_infinite_timeout() {
    let err = MeNotifyHost::timeout_for_test(f64::INFINITY).expect_err("infinite timeout should fail");

    assert!(matches!(err, MeNotifyError::HttpSpec(_)));
}

#[test]
fn http_request_rejects_absurd_timeout() {
    let err = MeNotifyHost::timeout_for_test(f64::MAX).expect_err("oversized timeout should fail");

    assert!(matches!(err, MeNotifyError::HttpSpec(_)));
}
