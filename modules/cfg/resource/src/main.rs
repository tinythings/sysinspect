use std::{
    collections::HashMap,
    fs,
    io::{self, Read, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::Duration,
};

use base64::{Engine, engine::general_purpose::STANDARD};
use clap::Parser;
use libmodcore::{
    init_mod_doc,
    modcli::ModuleCli,
    modinit::ModInterface,
    modlogger::{init_module_logger, take_logs},
    response::ModResponse,
    rtspec::RuntimeSpec,
    runtime::{ModRequest, get_call_args, send_call_response},
};
use libsysinspect::{
    cfg::mmconf::{CFG_MINION_RSA_PRV, DEFAULT_API_PORT, DEFAULT_MINION_MACHINE_ID, DEFAULT_MINION_MACHINE_ID_REL, DEFAULT_SYSINSPECT_ROOT},
    rsa::keys::{RsaKey, key_from_file, sign_data},
};
use reqwest::StatusCode;
use rsa::RsaPrivateKey;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

const HTTP_CONNECT_TIMEOUT_SECS: u64 = 5;
const HTTP_REQUEST_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Deserialize, Clone)]
struct StoreMetaResponse {
    sha256: String,
    size_bytes: u64,
    fmode: u32,
    created_unix: u64,
    expires_unix: Option<u64>,
    fname: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct StoreListEntry {
    sha256: String,
    size_bytes: u64,
    fmode: u32,
    created_unix: u64,
    expires_unix: Option<u64>,
    fname: Option<String>,
}

type JsonMap = HashMap<String, serde_json::Value>;

struct Ctx {
    cl: reqwest::blocking::Client,
    b: String,
    t: String,
    s: String,
    f: Option<String>,
    d: Option<String>,
    m: Option<String>,
    force: bool,
}

struct MinionAuth {
    minion_id: String,
    private_key: RsaPrivateKey,
}

#[derive(Debug, Deserialize, Clone)]
struct StoreMinionAuthResponse {
    access_token: String,
}

fn cfg_str(rq: &ModRequest, key: &str) -> Option<String> {
    rq.config().get(key).and_then(|v| v.as_string()).filter(|v| !v.is_empty())
}

fn arg_str(rq: &ModRequest, key: &str) -> Option<String> {
    rq.args().get(key).and_then(|v| v.as_string()).filter(|v| !v.is_empty())
}

fn arg_bool(rq: &ModRequest, key: &str) -> Option<bool> {
    rq.args().get(key).and_then(|v| v.as_bool())
}

fn parse_mode(mode: &str) -> Option<u32> {
    let raw = mode.trim();
    if raw.is_empty() {
        return None;
    }
    let norm = raw.trim_start_matches("0o").trim_start_matches("0O");
    u32::from_str_radix(norm, 8).ok()
}

fn file_sha256(path: &Path) -> io::Result<String> {
    let mut hasher = Sha256::new();
    let mut f = fs::File::open(path)?;
    let mut buf = [0u8; 0x4000];

    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn bytes_sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

fn timestamp_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

fn canonical_auth_material(method: &str, path: &str, query: &str, timestamp: &str, body_sha256: &str) -> String {
    format!("{method}\n{path}\n{query}\n{timestamp}\n{body_sha256}")
}

fn cfg_root(rq: &ModRequest) -> PathBuf {
    cfg_str(rq, "path.root").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(DEFAULT_SYSINSPECT_ROOT))
}

fn machine_id_path(rq: &ModRequest) -> PathBuf {
    if let Some(mid) = cfg_str(rq, "path.id") {
        if mid == "relative" {
            return cfg_root(rq).join(DEFAULT_MINION_MACHINE_ID_REL);
        }
        return PathBuf::from(mid);
    }
    let default = PathBuf::from(DEFAULT_MINION_MACHINE_ID);
    if default.exists() {
        default
    } else {
        cfg_root(rq).join(DEFAULT_MINION_MACHINE_ID_REL)
    }
}

fn load_minion_auth(rq: &ModRequest) -> Result<MinionAuth, String> {
    let minion_id = fs::read_to_string(machine_id_path(rq))
        .map_err(|e| format!("Unable to read minion identity: {e}"))?
        .trim()
        .to_string();
    if minion_id.is_empty() {
        return Err("Minion identity is empty".to_string());
    }

    let key_path = cfg_root(rq).join(CFG_MINION_RSA_PRV);
    let private_key = match key_from_file(key_path.to_str().unwrap_or_default()).map_err(|e| format!("Unable to load minion private key: {e}"))? {
        Some(RsaKey::Private(prk)) => prk,
        _ => return Err(format!("Minion private key not found at {}", key_path.display())),
    };

    Ok(MinionAuth { minion_id, private_key })
}

fn authed_request(
    client: &reqwest::blocking::Client, auth: &MinionAuth, method: reqwest::Method, url: &str, query: &[(&str, &str)], body_sha256: &str,
) -> Result<reqwest::blocking::RequestBuilder, String> {
    let mut parsed = reqwest::Url::parse(url).map_err(|e| format!("Invalid request URL '{url}': {e}"))?;
    if !query.is_empty() {
        let mut qp = parsed.query_pairs_mut();
        for (k, v) in query {
            qp.append_pair(k, v);
        }
    }
    let timestamp = timestamp_now();
    let material = canonical_auth_material(method.as_str(), parsed.path(), parsed.query().unwrap_or(""), &timestamp, body_sha256);
    let signature = sign_data(auth.private_key.clone(), material.as_bytes()).map_err(|e| format!("Unable to sign datastore request: {e}"))?;

    Ok(client
        .request(method, parsed)
        .header("X-Sysinspect-Minion-Id", auth.minion_id.clone())
        .header("X-Sysinspect-Timestamp", timestamp)
        .header("X-Sysinspect-Signature", STANDARD.encode(signature))
        .header("X-Sysinspect-Body-Sha256", body_sha256.to_string()))
}

fn bearer_request(client: &reqwest::blocking::Client, token: &str, method: reqwest::Method, url: &str) -> reqwest::blocking::RequestBuilder {
    client.request(method, url).bearer_auth(token)
}

fn api_base(rq: &ModRequest) -> String {
    let ip = cfg_str(rq, "master.ip").unwrap_or_else(|| "127.0.0.1".to_string());
    let port = arg_str(rq, "port")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(DEFAULT_API_PORT);
    let scheme = if arg_bool(rq, "tls").unwrap_or(true) { "https" } else { "http" };
    format!("{scheme}://{ip}:{port}")
}

fn bootstrap_datastore_token(client: &reqwest::blocking::Client, auth: &MinionAuth, base: &str) -> Result<String, String> {
    let url = format!("{base}/store/auth/minion");
    let rsp = authed_request(client, auth, reqwest::Method::POST, &url, &[], "")?
        .send()
        .map_err(|e| format!("store auth request failed: {e}"))?;
    if !rsp.status().is_success() {
        return Err(format!("store auth request failed: HTTP {}", rsp.status()));
    }
    let auth = rsp.json::<StoreMinionAuthResponse>().map_err(|e| format!("unable to parse store auth response: {e}"))?;
    if auth.access_token.trim().is_empty() {
        return Err("store auth response did not contain an access token".to_string());
    }
    Ok(auth.access_token)
}

fn resolve_meta(client: &reqwest::blocking::Client, token: &str, base: &str, src: &str) -> Result<Option<StoreMetaResponse>, String> {
    let url = format!("{base}/store/resolve");
    let req = bearer_request(client, token, reqwest::Method::GET, &url).query(&[("fname", src)]);

    let rsp = req.send().map_err(|e| format!("resolve request failed: {e}"))?;
    if rsp.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !rsp.status().is_success() {
        return Err(format!("resolve request failed: HTTP {}", rsp.status()));
    }

    rsp.json::<StoreMetaResponse>().map(Some).map_err(|e| format!("unable to parse resolve metadata: {e}"))
}

fn download_atomic(client: &reqwest::blocking::Client, token: &str, url: &str, dst: &Path) -> Result<(), String> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("failed to create destination parent '{}': {e}", parent.display()))?;
    }

    let req = bearer_request(client, token, reqwest::Method::GET, url);
    let mut rsp = req.send().map_err(|e| format!("download request failed: {e}"))?;
    if !rsp.status().is_success() {
        return Err(format!("download request failed: HTTP {}", rsp.status()));
    }

    let tmp = dst.with_extension("tmp");
    let mut f = fs::File::create(&tmp).map_err(|e| format!("failed to create temp file '{}': {e}", tmp.display()))?;
    io::copy(&mut rsp, &mut f).map_err(|e| format!("failed to write temp file '{}': {e}", tmp.display()))?;
    f.flush().map_err(|e| format!("failed to flush temp file '{}': {e}", tmp.display()))?;
    drop(f);

    fs::rename(&tmp, dst).map_err(|e| format!("failed to move temp file '{}' to '{}': {e}", tmp.display(), dst.display()))?;
    Ok(())
}

fn list_meta(client: &reqwest::blocking::Client, token: &str, base: &str, prefix: &str) -> Result<Vec<StoreListEntry>, String> {
    let url = format!("{base}/store/list");
    let req = bearer_request(client, token, reqwest::Method::GET, &url).query(&[("prefix", prefix)]);
    let rsp = req.send().map_err(|e| format!("list request failed: {e}"))?;
    if !rsp.status().is_success() {
        return Err(format!("list request failed: HTTP {}", rsp.status()));
    }
    rsp.json::<Vec<StoreListEntry>>().map_err(|e| format!("unable to parse store list response: {e}"))
}

fn finish(resp: &mut ModResponse, data: &mut JsonMap) {
    data.insert(RuntimeSpec::LogsSectionField.to_string(), json!(take_logs()));
    _ = resp.set_data(data.clone());
}

fn mk_ctx(rq: &ModRequest) -> Result<Ctx, String> {
    let s = arg_str(rq, "src").unwrap_or_default();
    if s.is_empty() {
        return Err("Argument \"src\" is required".to_string());
    }
    let cl = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECS))
        .danger_accept_invalid_certs(arg_bool(rq, "tls-accept-insecure").unwrap_or(false))
        .build()
        .map_err(|e| format!("Unable to initialize HTTP client: {e}"))?;

    let auth = load_minion_auth(rq)?;
    let base = api_base(rq);
    let token = bootstrap_datastore_token(&cl, &auth, &base)?;

    Ok(Ctx {
        cl,
        b: base,
        t: token,
        s,
        f: arg_str(rq, "file"),
        d: arg_str(rq, "dst"),
        m: arg_str(rq, "mode"),
        force: rq.has_option("force"),
    })
}

fn local_p(c: &Ctx) -> PathBuf {
    PathBuf::from(c.f.clone().or_else(|| c.d.clone()).unwrap_or_else(|| c.s.clone()))
}

fn do_push(c: &Ctx) -> Result<(bool, String, JsonMap), String> {
    let mut d = JsonMap::new();
    let p = local_p(c);
    if !p.is_file() {
        return Err(format!("Push source '{}' does not exist or is not a file", p.display()));
    }

    let hs = file_sha256(&p).map_err(|e| format!("Unable to checksum local file '{}': {e}", p.display()))?;
    if !c.force {
        match resolve_meta(&c.cl, &c.t, &c.b, &c.s)? {
            Some(m) if m.sha256 == hs => {
                d.insert("src".to_string(), json!(c.s));
                d.insert("sha256".to_string(), json!(m.sha256));
                d.insert("size_bytes".to_string(), json!(m.size_bytes));
                return Ok((false, "Resource is already up to date in datastore".to_string(), d));
            }
            _ => {}
        }
    }

    let b = fs::read(&p).map_err(|e| format!("Unable to read local file '{}': {e}", p.display()))?;
    let body_sha256 = bytes_sha256_hex(&b);
    let req = bearer_request(&c.cl, &c.t, reqwest::Method::POST, &format!("{}/store", c.b))
        .header("Content-Type", "application/octet-stream")
        .header("X-Filename", c.s.clone())
        .header("X-Sysinspect-Body-Sha256", body_sha256)
        .body(b);
    let rsp = req.send().map_err(|e| format!("Push request failed: {e}"))?;
    if !rsp.status().is_success() {
        let st = rsp.status();
        let bd = rsp.text().unwrap_or_default();
        return Err(format!("Push request failed: HTTP {st}: {bd}"));
    }

    let m = rsp.json::<StoreMetaResponse>().map_err(|e| format!("Unable to parse push metadata: {e}"))?;
    d.insert("src".to_string(), json!(c.s));
    d.insert("sha256".to_string(), json!(m.sha256));
    d.insert("size_bytes".to_string(), json!(m.size_bytes));
    d.insert("created_unix".to_string(), json!(m.created_unix));
    d.insert("expires_unix".to_string(), json!(m.expires_unix));
    d.insert("fname".to_string(), json!(m.fname));
    Ok((true, "Resource pushed to datastore".to_string(), d))
}

fn do_pull(c: &Ctx) -> Result<(bool, String, JsonMap), String> {
    let mut d = JsonMap::new();
    let p = local_p(c);
    let m = match resolve_meta(&c.cl, &c.t, &c.b, &c.s)? {
        Some(m) => m,
        None => return Err(format!("Resource '{}' was not found in datastore", c.s)),
    };
    if p.exists() && !c.force {
        match file_sha256(&p) {
            Ok(h) if h == m.sha256 => {
                d.insert("src".to_string(), json!(c.s));
                d.insert("dst".to_string(), json!(p));
                d.insert("sha256".to_string(), json!(m.sha256));
                d.insert("size_bytes".to_string(), json!(m.size_bytes));
                return Ok((false, format!("Resource '{}' already matches checksum", p.display()), d));
            }
            Ok(_) => {}
            Err(e) => return Err(format!("Unable to checksum destination file '{}': {e}", p.display())),
        }
    }

    let url = format!("{}/store/{}/blob", c.b, m.sha256);
    download_atomic(&c.cl, &c.t, &url, &p)?;
    let md = c.m.as_deref().and_then(parse_mode).unwrap_or(m.fmode & 0o7777);
    fs::set_permissions(&p, fs::Permissions::from_mode(md))
        .map_err(|e| format!("Downloaded resource but failed to set mode on '{}': {e}", p.display()))?;

    d.insert("src".to_string(), json!(c.s));
    d.insert("dst".to_string(), json!(p));
    d.insert("sha256".to_string(), json!(m.sha256));
    d.insert("size_bytes".to_string(), json!(m.size_bytes));
    d.insert("created_unix".to_string(), json!(m.created_unix));
    d.insert("expires_unix".to_string(), json!(m.expires_unix));
    d.insert("fname".to_string(), json!(m.fname));
    d.insert("mode".to_string(), json!(format!("{:04o}", md)));
    Ok((true, "Resource downloaded from datastore".to_string(), d))
}

fn do_sync_dir(c: &Ctx) -> Result<(bool, String, JsonMap), String> {
    let mut d = JsonMap::new();
    let dst_dir = c
        .f
        .clone()
        .or_else(|| c.d.clone())
        .map(PathBuf::from)
        .ok_or_else(|| "Argument \"file\" or \"dst\" is required for sync-dir".to_string())?;
    fs::create_dir_all(&dst_dir).map_err(|e| format!("Unable to create destination directory '{}': {e}", dst_dir.display()))?;

    let metas = list_meta(&c.cl, &c.t, &c.b, &c.s)?;
    let mut changed = false;
    let mut synced = 0usize;
    for meta in metas {
        let Some(fname) = meta.fname.as_deref() else {
            continue;
        };
        let Some(name) = Path::new(fname).file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let dst = dst_dir.join(name);
        if dst.exists() && !c.force {
            if let Ok(h) = file_sha256(&dst)
                && h == meta.sha256
            {
                synced += 1;
                continue;
            }
        }

        let url = format!("{}/store/{}/blob", c.b, meta.sha256);
        download_atomic(&c.cl, &c.t, &url, &dst)?;
        let md = c.m.as_deref().and_then(parse_mode).unwrap_or(meta.fmode & 0o7777);
        fs::set_permissions(&dst, fs::Permissions::from_mode(md))
            .map_err(|e| format!("Downloaded resource but failed to set mode on '{}': {e}", dst.display()))?;
        changed = true;
        synced += 1;
    }

    d.insert("src".to_string(), json!(c.s));
    d.insert("dst".to_string(), json!(dst_dir));
    d.insert("synced".to_string(), json!(synced));
    Ok((changed, format!("Pubring synced ({} files)", synced), d))
}

fn run(_cli: &ModuleCli, rq: &ModRequest) -> ModResponse {
    let mut resp = ModResponse::new_cm();
    let mut data = JsonMap::new();

    let push = rq.has_option("push");
    let pull = rq.has_option("pull");
    let sync_dir = rq.has_option("sync-dir");
    let selected = push as u8 + pull as u8 + sync_dir as u8;
    if selected > 1 {
        resp.set_message("Configuration error: cannot combine push, pull, and sync-dir options");
        finish(&mut resp, &mut data);
        return resp;
    }
    if selected == 0 {
        resp.set_message("Configuration error: must have one of push, pull, or sync-dir option");
        finish(&mut resp, &mut data);
        return resp;
    }

    let c = match mk_ctx(rq) {
        Ok(c) => c,
        Err(e) => {
            resp.set_message(&e);
            finish(&mut resp, &mut data);
            return resp;
        }
    };

    let out = if push {
        do_push(&c)
    } else if pull {
        do_pull(&c)
    } else {
        do_sync_dir(&c)
    };
    match out {
        Ok((ch, msg, mut d)) => {
            resp.set_retcode(0);
            _ = resp.cm_set_changed(ch);
            resp.set_message(&msg);
            data.extend(d.drain());
        }
        Err(e) => {
            resp.set_message(&e);
        }
    }

    finish(&mut resp, &mut data);
    resp
}

fn main() {
    let mod_doc = init_mod_doc!(ModInterface);
    init_module_logger(mod_doc.name());

    let cli = ModuleCli::parse();

    // CLI calls from the terminal directly
    if cli.is_manual() {
        print!("{}", mod_doc.help());
        return;
    }

    // Runtime call (integrated via JSON protocol)
    match get_call_args() {
        Ok(rq) => match send_call_response(&run(&cli, &rq)) {
            Ok(_) => {}
            Err(err) => println!("Runtime error: {err}"),
        },
        Err(err) => println!("Arguments error: {err}"),
    }
}
