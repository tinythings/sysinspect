use libmodpak::{SysInspectModPak, SysInspectModPakMinion, mpk::ModPakRepoIndex};
use libsysinspect::{
    cfg::mmconf::MinionConfig,
    traits::{TraitUpdateRequest, ensure_master_traits_file},
};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

fn write_file(path: &Path, data: &[u8]) {
    fs::create_dir_all(path.parent().expect("parent should exist")).expect("parent dir should be created");
    fs::write(path, data).expect("file should be written");
}

fn add_script_module(root: &Path, name: &str, body: &str) {
    let subpath = name.replace('.', "/");
    write_file(&root.join("script/any/noarch").join(&subpath), body.as_bytes());
}

fn set_script_modules(root: &Path, modules: &[&str]) {
    let mut index = if root.join("mod.index").exists() {
        ModPakRepoIndex::from_yaml(&fs::read_to_string(root.join("mod.index")).expect("mod.index should read")).expect("mod.index should deserialize")
    } else {
        ModPakRepoIndex::new()
    };
    for module in modules {
        index
            .index_module(module, &module.replace('.', "/"), "any", "noarch", "demo module", false, "deadbeef", None, None)
            .expect("module should index");
    }
    fs::write(root.join("mod.index"), index.to_yaml().expect("mod.index should serialize")).expect("mod.index should write");
}

fn add_library_tree(repo: &mut SysInspectModPak, root: &Path, rel: &str) {
    let file = root.join("lib").join(rel);
    write_file(&file, rel.as_bytes());
    repo.add_library(root.to_path_buf()).expect("library should be added");
}

async fn start_fileserver(root: PathBuf) -> (u16, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener should bind");
    let port = listener.local_addr().expect("listener addr should exist").port();
    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else { break };
            let root = root.clone();
            tokio::spawn(async move {
                let mut buf = [0_u8; 4096];
                let Ok(n) = stream.read(&mut buf).await else { return };
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req.lines().next().and_then(|line| line.split_whitespace().nth(1)).unwrap_or("/");
                let file = root.join(path.trim_start_matches('/'));
                let response = match fs::read(&file) {
                    Ok(body) => format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes(),
                    Err(_) => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
                };
                let body = fs::read(&file).unwrap_or_default();
                let _ = stream.write_all(&response).await;
                if !body.is_empty() {
                    let _ = stream.write_all(&body).await;
                }
            });
        }
    });
    (port, handle)
}

fn configured_minion(root: &Path, share: &Path, port: u16) -> MinionConfig {
    let mut cfg = MinionConfig::default();
    cfg.set_root_dir(root.to_str().expect("root path should be valid"));
    cfg.set_sharelib_path(share.to_str().expect("share path should be valid"));
    cfg.set_master_ip("127.0.0.1");
    cfg.set_master_fileserver_port(port.into());
    cfg
}

#[tokio::test]
async fn narrow_profile_syncs_only_allowed_artifacts_and_removes_old_ones() {
    let master = tempfile::tempdir().expect("master tempdir should be created");
    let mut repo = SysInspectModPak::new(master.path().join("data/repo")).expect("repo should be created");
    let libs = tempfile::tempdir().expect("library tempdir should be created");
    add_library_tree(&mut repo, libs.path(), "runtime/lua/alpha.lua");
    add_library_tree(&mut repo, libs.path(), "runtime/lua/beta.lua");
    add_script_module(&master.path().join("data/repo"), "alpha.demo", "# alpha");
    add_script_module(&master.path().join("data/repo"), "beta.demo", "# beta");
    set_script_modules(&master.path().join("data/repo"), &["alpha.demo", "beta.demo"]);
    repo.new_profile("Alpha").expect("Alpha should be created");
    repo.new_profile("Beta").expect("Beta should be created");
    repo.add_profile_matches("Alpha", vec!["alpha.demo".to_string()], false).expect("Alpha module selector should be added");
    repo.add_profile_matches("Alpha", vec!["lib/runtime/lua/alpha.lua".to_string()], true).expect("Alpha library selector should be added");
    repo.add_profile_matches("Beta", vec!["beta.demo".to_string()], false).expect("Beta module selector should be added");
    repo.add_profile_matches("Beta", vec!["lib/runtime/lua/beta.lua".to_string()], true).expect("Beta library selector should be added");

    let (port, server) = start_fileserver(master.path().join("data")).await;
    let minion = tempfile::tempdir().expect("minion tempdir should be created");
    let share = tempfile::tempdir().expect("share tempdir should be created");
    let cfg = configured_minion(minion.path(), share.path(), port);
    fs::create_dir_all(cfg.traits_dir()).expect("traits dir should be created");
    ensure_master_traits_file(&cfg).expect("master traits file should exist");
    TraitUpdateRequest::from_context(r#"{"op":"set","traits":{"minion.profile":["Alpha"]}}"#)
        .expect("set request should parse")
        .apply(&cfg)
        .expect("set request should apply");

    SysInspectModPakMinion::new(cfg.clone()).sync().await.expect("first sync should work");
    assert!(share.path().join("modules/alpha/demo").exists());
    assert!(!share.path().join("modules/beta/demo").exists());
    assert!(share.path().join("lib/lib/runtime/lua/alpha.lua").exists());
    assert!(!share.path().join("lib/lib/runtime/lua/beta.lua").exists());

    TraitUpdateRequest::from_context(r#"{"op":"set","traits":{"minion.profile":["Beta"]}}"#)
        .expect("set request should parse")
        .apply(&cfg)
        .expect("set request should apply");
    SysInspectModPakMinion::new(cfg).sync().await.expect("second sync should work");
    assert!(!share.path().join("modules/alpha/demo").exists());
    assert!(share.path().join("modules/beta/demo").exists());
    assert!(!share.path().join("lib/lib/runtime/lua/alpha.lua").exists());
    assert!(share.path().join("lib/lib/runtime/lua/beta.lua").exists());

    server.abort();
}

#[tokio::test]
async fn overlapping_multi_profile_sync_merges_by_union_and_dedup() {
    let master = tempfile::tempdir().expect("master tempdir should be created");
    let repo = SysInspectModPak::new(master.path().join("data/repo")).expect("repo should be created");
    add_script_module(&master.path().join("data/repo"), "alpha.demo", "# alpha");
    add_script_module(&master.path().join("data/repo"), "beta.demo", "# beta");
    add_script_module(&master.path().join("data/repo"), "gamma.demo", "# gamma");
    set_script_modules(&master.path().join("data/repo"), &["alpha.demo", "beta.demo", "gamma.demo"]);
    repo.new_profile("One").expect("One should be created");
    repo.new_profile("Two").expect("Two should be created");
    repo.add_profile_matches("One", vec!["alpha.demo".to_string(), "beta.demo".to_string()], false).expect("One selectors should be added");
    repo.add_profile_matches("Two", vec!["beta.demo".to_string(), "gamma.demo".to_string()], false).expect("Two selectors should be added");

    let (port, server) = start_fileserver(master.path().join("data")).await;
    let minion = tempfile::tempdir().expect("minion tempdir should be created");
    let share = tempfile::tempdir().expect("share tempdir should be created");
    let cfg = configured_minion(minion.path(), share.path(), port);
    fs::create_dir_all(cfg.traits_dir()).expect("traits dir should be created");
    ensure_master_traits_file(&cfg).expect("master traits file should exist");
    TraitUpdateRequest::from_context(r#"{"op":"set","traits":{"minion.profile":["One","Two","One"]}}"#)
        .expect("set request should parse")
        .apply(&cfg)
        .expect("set request should apply");

    SysInspectModPakMinion::new(cfg).sync().await.expect("sync should work");
    assert!(share.path().join("modules/alpha/demo").exists());
    assert!(share.path().join("modules/beta/demo").exists());
    assert!(share.path().join("modules/gamma/demo").exists());

    server.abort();
}

#[tokio::test]
async fn sync_fails_if_effective_profiles_are_missing_from_profiles_index() {
    let master = tempfile::tempdir().expect("master tempdir should be created");
    let repo = SysInspectModPak::new(master.path().join("data/repo")).expect("repo should be created");
    add_script_module(&master.path().join("data/repo"), "alpha.demo", "# alpha");
    set_script_modules(&master.path().join("data/repo"), &["alpha.demo"]);
    repo.new_profile("Existing").expect("Existing should be created");
    repo.add_profile_matches("Existing", vec!["alpha.demo".to_string()], false).expect("Existing selector should be added");

    let (port, server) = start_fileserver(master.path().join("data")).await;
    let minion = tempfile::tempdir().expect("minion tempdir should be created");
    let share = tempfile::tempdir().expect("share tempdir should be created");
    let cfg = configured_minion(minion.path(), share.path(), port);
    fs::create_dir_all(cfg.traits_dir()).expect("traits dir should be created");
    ensure_master_traits_file(&cfg).expect("master traits file should exist");
    TraitUpdateRequest::from_context(r#"{"op":"set","traits":{"minion.profile":["Missing"]}}"#)
        .expect("set request should parse")
        .apply(&cfg)
        .expect("set request should apply");

    let err = SysInspectModPakMinion::new(cfg).sync().await.expect_err("sync should fail when effective profiles are missing");
    assert!(err.to_string().contains("Missing"));

    server.abort();
}

#[tokio::test]
async fn sync_rejects_profile_paths_with_traversal_components() {
    let master = tempfile::tempdir().expect("master tempdir should be created");
    fs::create_dir_all(master.path().join("data")).expect("data dir should be created");
    fs::write(master.path().join("data/profiles.index"), "profiles:\n  Escape:\n    file: ../escape.profile\n    checksum: deadbeef\n")
        .expect("profiles index should be written");

    let (port, server) = start_fileserver(master.path().join("data")).await;
    let minion = tempfile::tempdir().expect("minion tempdir should be created");
    let share = tempfile::tempdir().expect("share tempdir should be created");
    let cfg = configured_minion(minion.path(), share.path(), port);
    fs::create_dir_all(cfg.traits_dir()).expect("traits dir should be created");
    ensure_master_traits_file(&cfg).expect("master traits file should exist");
    TraitUpdateRequest::from_context(r#"{"op":"set","traits":{"minion.profile":["Escape"]}}"#)
        .expect("set request should parse")
        .apply(&cfg)
        .expect("set request should apply");

    let err = SysInspectModPakMinion::new(cfg).sync().await.expect_err("sync should fail on path traversal");
    assert!(err.to_string().contains("Invalid profile path"));

    server.abort();
}

#[tokio::test]
async fn sync_fails_if_downloaded_profile_checksum_does_not_match_index() {
    let master = tempfile::tempdir().expect("master tempdir should be created");
    let repo = SysInspectModPak::new(master.path().join("data/repo")).expect("repo should be created");
    add_script_module(&master.path().join("data/repo"), "alpha.demo", "# alpha");
    set_script_modules(&master.path().join("data/repo"), &["alpha.demo"]);
    repo.new_profile("Broken").expect("Broken should be created");
    repo.add_profile_matches("Broken", vec!["alpha.demo".to_string()], false).expect("Broken selector should be added");
    fs::write(master.path().join("data/profiles.index"), "profiles:\n  Broken:\n    file: broken.profile\n    checksum: deadbeef\n")
        .expect("profiles index should be overwritten");

    let (port, server) = start_fileserver(master.path().join("data")).await;
    let minion = tempfile::tempdir().expect("minion tempdir should be created");
    let share = tempfile::tempdir().expect("share tempdir should be created");
    let cfg = configured_minion(minion.path(), share.path(), port);
    fs::create_dir_all(cfg.traits_dir()).expect("traits dir should be created");
    ensure_master_traits_file(&cfg).expect("master traits file should exist");
    TraitUpdateRequest::from_context(r#"{"op":"set","traits":{"minion.profile":["Broken"]}}"#)
        .expect("set request should parse")
        .apply(&cfg)
        .expect("set request should apply");

    let err = SysInspectModPakMinion::new(cfg).sync().await.expect_err("sync should fail on profile checksum mismatch");
    assert!(err.to_string().contains("Checksum mismatch for profile"));

    server.abort();
}
