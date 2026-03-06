use std::{
    collections::HashMap,
    fs,
    io::{self, Read, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

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
use libsysinspect::cfg::mmconf::DEFAULT_API_PORT;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

#[derive(Debug, Deserialize, Clone)]
struct StoreMetaResponse {
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
    s: String,
    f: Option<String>,
    d: Option<String>,
    m: Option<String>,
    force: bool,
}

fn cfg_str(rq: &ModRequest, key: &str) -> Option<String> {
    rq.config().get(key).and_then(|v| v.as_string()).filter(|v| !v.is_empty())
}

fn arg_str(rq: &ModRequest, key: &str) -> Option<String> {
    rq.args().get(key).and_then(|v| v.as_string()).filter(|v| !v.is_empty())
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

fn api_base(rq: &ModRequest) -> String {
    let ip = cfg_str(rq, "master.ip").unwrap_or_else(|| "127.0.0.1".to_string());
    format!("http://{ip}:{}", DEFAULT_API_PORT)
}

fn resolve_meta(client: &reqwest::blocking::Client, base: &str, src: &str) -> Result<Option<StoreMetaResponse>, String> {
    let url = format!("{base}/store/resolve");
    let req = client.get(url).query(&[("fname", src)]);

    let rsp = req.send().map_err(|e| format!("resolve request failed: {e}"))?;
    if rsp.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !rsp.status().is_success() {
        return Err(format!("resolve request failed: HTTP {}", rsp.status()));
    }

    rsp.json::<StoreMetaResponse>().map(Some).map_err(|e| format!("unable to parse resolve metadata: {e}"))
}

fn download_atomic(client: &reqwest::blocking::Client, url: &str, dst: &Path) -> Result<(), String> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("failed to create destination parent '{}': {e}", parent.display()))?;
    }

    let req = client.get(url);
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

fn finish(resp: &mut ModResponse, data: &mut JsonMap) {
    data.insert(RuntimeSpec::LogsSectionField.to_string(), json!(take_logs()));
    _ = resp.set_data(data.clone());
}

fn mk_ctx(rq: &ModRequest) -> Result<Ctx, String> {
    let s = arg_str(rq, "src").unwrap_or_default();
    if s.is_empty() {
        return Err("Argument \"src\" is required".to_string());
    }
    Ok(Ctx {
        cl: reqwest::blocking::Client::new(),
        b: api_base(rq),
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
        match resolve_meta(&c.cl, &c.b, &c.s)? {
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
    let req = c
        .cl
        .post(format!("{}/store", c.b))
        .header("Content-Type", "application/octet-stream")
        .header("X-Filename", c.s.clone())
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
    let m = match resolve_meta(&c.cl, &c.b, &c.s)? {
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
    download_atomic(&c.cl, &url, &p)?;
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

fn run(_cli: &ModuleCli, rq: &ModRequest) -> ModResponse {
    let mut resp = ModResponse::new_cm();
    let mut data = JsonMap::new();

    if rq.has_option("push") && rq.has_option("pull") {
        resp.set_message("Configuration error: cannot have both push and pull options");
        finish(&mut resp, &mut data);
        return resp;
    }
    if !rq.has_option("push") && !rq.has_option("pull") {
        resp.set_message("Configuration error: must have either push or pull option");
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

    let out = if rq.has_option("push") { do_push(&c) } else { do_pull(&c) };
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
