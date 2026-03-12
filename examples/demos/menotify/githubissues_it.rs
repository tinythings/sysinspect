use libmenotify::{MeNotifyContext, MeNotifyEventBuilder, MeNotifyProgram, MeNotifyRunner, MeNotifyRuntime};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    sync::Mutex,
    thread,
    time::Duration,
};

fn spawn_github_issues_server(first: &'static str, second: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener.local_addr().expect("addr should resolve");

    thread::spawn(move || {
        for body in [first, second] {
            let (mut stream, _) = listener.accept().expect("server should accept");
            let mut buf = [0u8; 8192];
            _ = stream.read(&mut buf).expect("server should read");
            stream
                .write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}", body.len(), body).as_bytes())
                .expect("server should write");
            stream.flush().expect("server should flush");
        }
    });

    format!("http://{addr}")
}

fn copy_demo_script(dst_root: &Path) {
    fs::create_dir_all(dst_root).expect("script root should be created");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/demos/menotify/lib/sensors/lua/githubissues.lua"),
        dst_root.join("githubissues.lua"),
    )
    .expect("demo script should copy");
}

#[test]
fn githubissues_demo_seeds_then_emits_new_issue() {
    let api = spawn_github_issues_server(
        r#"[{"number":1,"title":"First issue","body":"seed","state":"open","html_url":"https://example.test/1","url":"https://api.example.test/1","created_at":"2026-03-11T10:00:00Z","updated_at":"2026-03-11T10:00:00Z","user":{"login":"alice"}}]"#,
        r#"[{"number":2,"title":"Second issue","body":"new body","state":"open","html_url":"https://example.test/2","url":"https://api.example.test/2","created_at":"2026-03-11T10:05:00Z","updated_at":"2026-03-11T10:05:00Z","user":{"login":"bob"}},{"number":1,"title":"First issue","body":"seed","state":"open","html_url":"https://example.test/1","url":"https://api.example.test/1","created_at":"2026-03-11T10:00:00Z","updated_at":"2026-03-11T10:00:00Z","user":{"login":"alice"}}]"#,
    );

    let tmp = tempfile::tempdir().expect("tempdir should be created");
    copy_demo_script(&tmp.path().join("lib/sensors/lua"));

    let runtime =
        MeNotifyRuntime::with_sharelib_root("github-public-issues".to_string(), "menotify.githubissues".to_string(), tmp.path().to_path_buf());
    let runner = MeNotifyRunner::new(
        MeNotifyProgram::new(&runtime).expect("program should load"),
        MeNotifyContext::new(
            "github-public-issues",
            "menotify.githubissues",
            "githubissues",
            &[],
            &serde_yaml::from_str(&format!("owner: acme\nrepo: demo\napi: {api}\nstate: open\nper_page: 20\nuser_agent: menotify-test\n"))
                .expect("yaml should parse"),
            Some(Duration::from_secs(1)),
        ),
    );
    let out = Mutex::new(Vec::new());
    let builder = MeNotifyEventBuilder::new("github-public-issues", "menotify.githubissues", None);

    runner.run_tick_with_emit(&|ev| out.lock().expect("lock should work").push(ev), &builder).expect("first tick should seed");
    assert!(out.lock().expect("lock should work").is_empty());

    runner.run_tick_with_emit(&|ev| out.lock().expect("lock should work").push(ev), &builder).expect("second tick should emit");

    let events = out.lock().expect("lock should work");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["eid"], "github-public-issues|menotify.githubissues|opened@2|0");
    assert_eq!(events[0]["data"]["number"], 2);
    assert_eq!(events[0]["data"]["title"], "Second issue");
    assert_eq!(events[0]["data"]["body"], "new body");
    assert_eq!(events[0]["data"]["user"], "bob");
}
