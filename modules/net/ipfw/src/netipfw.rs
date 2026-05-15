use libmodcore::{
    response::ModResponse,
    runtime::{self, ModRequest},
};
use std::io::Write;
use std::process::{Command, Stdio};

pub(crate) fn detect_backend() -> Option<&'static str> {
    let candidates: &[(&str, &[&str])] = &[
        ("pf", &["pfctl", "-s", "info"]),
        ("ipfw", &["ipfw", "list"]),
        ("nftables", &["nft", "list", "ruleset"]),
        ("iptables", &["iptables", "-L", "-n"]),
    ];
    candidates.iter().find_map(|(name, args)| Command::new(args[0]).args(&args[1..]).output().ok().filter(|o| o.status.success()).map(|_| *name))
}

fn exec(cmd: &str, args: &[&str]) -> Result<(i32, String, String), String> {
    let out = Command::new(cmd).args(args).output().map_err(|e| format!("{}: {e}", cmd))?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    Ok((out.status.code().unwrap_or(1), stdout, stderr))
}

fn exec_stdin(cmd: &str, args: &[&str], stdin: &str) -> Result<(i32, String, String), String> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{cmd}: {e}"))?;

    if let Some(mut sin) = child.stdin.take() {
        let _ = sin.write_all(stdin.as_bytes());
    }
    let out = child.wait_with_output().map_err(|e| format!("{cmd}: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    Ok((out.status.code().unwrap_or(1), stdout, stderr))
}

fn backend_list(backend: &str) -> Result<String, String> {
    match backend {
        "pf" => {
            let (_, out, _) = exec("pfctl", &["-s", "rules"])?;
            Ok(out)
        }
        "ipfw" => {
            let (_, out, _) = exec("ipfw", &["list"])?;
            Ok(out)
        }
        "nftables" => {
            let (_, out, _) = exec("nft", &["list", "ruleset"])?;
            Ok(out)
        }
        "iptables" => {
            let (_, out, _) = exec("iptables", &["-L", "-n"])?;
            Ok(out)
        }
        _ => Err(format!("unknown backend: {backend}")),
    }
}

fn backend_flush(backend: &str) -> Result<String, String> {
    match backend {
        "pf" => {
            let (_, out, _) = exec("pfctl", &["-F", "all"])?;
            Ok(out)
        }
        "ipfw" => {
            let (_, out, _) = exec("ipfw", &["-f", "flush"])?;
            Ok(out)
        }
        "nftables" => {
            let (_, out, _) = exec("nft", &["flush", "ruleset"])?;
            Ok(out)
        }
        "iptables" => {
            let _ = exec("iptables", &["-F"])?;
            let _ = exec("iptables", &["-X"])?;
            Ok(String::new())
        }
        _ => Err(format!("unknown backend: {backend}")),
    }
}

fn backend_add(backend: &str, rule_text: &str) -> Result<(i32, String, String), String> {
    match backend {
        "pf" => {
            let tmp = format!("/tmp/ipfw-pf-{}", std::process::id());
            std::fs::write(&tmp, format!("{rule_text}\n")).map_err(|e| e.to_string())?;
            let result = exec("pfctl", &["-f", &tmp]);
            let _ = std::fs::remove_file(&tmp);
            result
        }
        "ipfw" => {
            let parts: Vec<&str> = rule_text.splitn(2, ' ').collect();
            if parts.len() < 2 {
                return Err("invalid ipfw rule".to_string());
            }
            exec("ipfw", &["-q", parts[0], parts[1]])
        }
        "nftables" => exec_stdin("nft", &["-f", "-"], &format!("{rule_text}\n")),
        "iptables" => {
            let parts: Vec<&str> = rule_text.split_whitespace().collect();
            exec("iptables", &parts)
        }
        _ => Err(format!("unknown backend: {backend}")),
    }
}

pub(crate) fn translate_rule(backend: &str, args: &serde_json::Value) -> Result<String, String> {
    let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("allow");
    let proto = args.get("protocol").and_then(|v| v.as_str()).unwrap_or("tcp");
    let port = args.get("port").and_then(|v| v.as_str()).unwrap_or("");
    let port_range = args.get("port-range").and_then(|v| v.as_str()).unwrap_or("");
    let src = args.get("source").and_then(|v| v.as_str()).unwrap_or("any");
    let dst = args.get("destination").and_then(|v| v.as_str()).unwrap_or("any");
    let iface = args.get("interface").and_then(|v| v.as_str()).unwrap_or("");
    let dir = args.get("direction").and_then(|v| v.as_str()).unwrap_or("in");

    let port_spec = if !port_range.is_empty() { port_range } else { port };

    match backend {
        "pf" => {
            let mut rule = String::from(if action == "allow" { "pass" } else { "block" });
            if dir == "out" {
                rule.push_str(" out");
            } else {
                rule.push_str(" in");
            }
            rule.push_str(" quick");
            if proto != "any" {
                rule.push_str(&format!(" proto {proto}"));
            }
            if src != "any" {
                rule.push_str(&format!(" from {src}"));
            } else {
                rule.push_str(" from any");
            }
            if dst != "any" {
                rule.push_str(&format!(" to {dst}"));
            } else {
                rule.push_str(" to any");
            }
            if !port_spec.is_empty() {
                rule.push_str(&format!(" port {port_spec}"));
            }
            if !iface.is_empty() {
                rule.push_str(&format!(" on {iface}"));
            }
            Ok(rule)
        }
        "ipfw" => {
            let act = if action == "allow" { "allow" } else { "deny" };
            let mut rule = format!("add {act} {proto}");
            if src != "any" {
                rule.push_str(&format!(" from {src}"));
            } else {
                rule.push_str(" from any");
            }
            if dst != "any" {
                rule.push_str(&format!(" to {dst}"));
            } else {
                rule.push_str(" to any");
            }
            if !port_spec.is_empty() {
                rule.push_str(&format!(" dst-port {port_spec}"));
            }
            if !iface.is_empty() {
                rule.push_str(&format!(" via {iface}"));
            }
            if dir == "out" {
                rule.push_str(" out");
            } else {
                rule.push_str(" in");
            }
            Ok(rule)
        }
        "nftables" => {
            let act = if action == "allow" { "accept" } else { "drop" };
            let mut rule = String::from("add rule inet filter");
            if dir == "out" {
                rule.push_str(" output");
            } else {
                rule.push_str(" input");
            }
            if proto != "any" {
                rule.push_str(&format!(" {proto}"));
            }
            if !port_spec.is_empty() {
                rule.push_str(&format!(" dport {port_spec}"));
            }
            if src != "any" {
                rule.push_str(&format!(" ip saddr {src}"));
            }
            if dst != "any" {
                rule.push_str(&format!(" ip daddr {dst}"));
            }
            if !iface.is_empty() {
                rule.push_str(&format!(" iifname {iface}"));
            }
            rule.push_str(&format!(" {act}"));
            Ok(rule)
        }
        "iptables" => {
            let chain = if dir == "out" { "OUTPUT" } else { "INPUT" };
            let jump = if action == "allow" { "ACCEPT" } else { "DROP" };
            let mut rule = format!("-A {chain}");
            if proto != "any" {
                rule.push_str(&format!(" -p {proto}"));
            }
            if !port_spec.is_empty() {
                rule.push_str(&format!(" --dport {port_spec}"));
            }
            if src != "any" {
                rule.push_str(&format!(" -s {src}"));
            }
            if dst != "any" {
                rule.push_str(&format!(" -d {dst}"));
            }
            if !iface.is_empty() {
                rule.push_str(&format!(" -i {iface}"));
            }
            rule.push_str(&format!(" -j {jump}"));
            Ok(rule)
        }
        _ => Err(format!("Unsupported backend: {backend}")),
    }
}

fn parse_rule_args(rt: &ModRequest) -> serde_json::Value {
    serde_json::json!({
        "action": runtime::get_arg(rt, "action"),
        "protocol": runtime::get_arg(rt, "protocol"),
        "port": runtime::get_arg(rt, "port"),
        "port-range": runtime::get_arg(rt, "port-range"),
        "source": runtime::get_arg(rt, "source"),
        "destination": runtime::get_arg(rt, "destination"),
        "interface": runtime::get_arg(rt, "interface"),
        "direction": runtime::get_arg(rt, "direction"),
        "native": runtime::get_arg(rt, "native"),
    })
}

pub fn run(rt: &ModRequest) -> ModResponse {
    let mut resp = runtime::new_call_response();
    let dry_run = runtime::get_opt(rt, "dry-run");
    let do_flush = runtime::get_opt(rt, "flush");
    let do_check = runtime::get_opt(rt, "check");
    let do_present = runtime::get_opt(rt, "present");
    let do_absent = runtime::get_opt(rt, "absent");

    if !do_check && !do_present && !do_absent && !do_flush {
        resp.set_retcode(1);
        resp.set_message("No operation specified. Use --check, --present, --absent, or --flush");
        return resp;
    }

    let backend = match runtime::get_arg(rt, "backend") {
        b if !b.is_empty() => Some(b),
        _ => detect_backend().map(|s| s.to_string()),
    };

    let effective_backend = if dry_run {
        // Dry-run: pick any detected backend for rule preview.
        // If none detected, use pf as reference renderer.
        backend.clone().unwrap_or_else(|| "pf".to_string())
    } else {
        match backend {
            Some(b) => b,
            None => {
                resp.set_retcode(1);
                resp.set_message("No supported firewall backend detected (tried: pf, ipfw, nftables, iptables). Use --backend to force one.");
                return resp;
            }
        }
    };

    // ---------- check ----------
    if do_check {
        if dry_run {
            resp.set_message(&format!("[dry-run] would list rules for {effective_backend}"));
            return resp;
        }
        match backend_list(&effective_backend) {
            Ok(rules) => {
                resp.set_retcode(0);
                resp.set_message(&format!("{effective_backend} rules listed"));
                let _ = resp.set_data(serde_json::json!({"rules": rules.lines().collect::<Vec<_>>()}));
            }
            Err(e) => {
                resp.set_retcode(1);
                resp.set_message(&format!("Failed to list rules: {e}"));
            }
        }
        return resp;
    }

    // ---------- flush ----------
    if do_flush {
        if dry_run {
            resp.set_message(&format!("[dry-run] would flush all rules for {effective_backend}"));
            return resp;
        }
        match backend_flush(&effective_backend) {
            Ok(_) => {
                resp.set_message(&format!("All {effective_backend} rules flushed"));
            }
            Err(e) => {
                resp.set_retcode(1);
                resp.set_message(&format!("Failed to flush rules: {e}"));
            }
        }
        return resp;
    }

    // ---------- present / absent ----------
    // Build the rule expression from args
    let raw_native = runtime::get_arg(rt, "native");
    let args_val = parse_rule_args(rt);
    let native_from_arg = if !raw_native.is_empty() {
        serde_json::from_str::<serde_json::Value>(&raw_native)
            .ok()
            .and_then(|n| n.get(effective_backend.as_str()).and_then(|v| v.as_str()).map(|s| s.to_string()))
    } else {
        None
    };

    let rule_text = if let Some(raw) = native_from_arg {
        raw
    } else {
        match translate_rule(&effective_backend, &args_val) {
            Ok(r) => r,
            Err(e) => {
                resp.set_retcode(1);
                resp.set_message(&e);
                return resp;
            }
        }
    };

    if dry_run {
        let op = if do_present { "add" } else { "remove" };
        resp.set_message(&format!("[dry-run] would {op} rule ({effective_backend}): {rule_text}"));
        return resp;
    }

    if do_present {
        match backend_add(&effective_backend, &rule_text) {
            Ok((0, _, _)) => {
                resp.set_message(&format!("Firewall rule added ({effective_backend})"));
                let _ = resp.set_data(serde_json::json!({"rule": rule_text, "backend": effective_backend}));
            }
            Ok((code, _, stderr)) => {
                resp.set_retcode(code);
                resp.set_message(format!("Firewall rule failed: {stderr}").trim());
            }
            Err(e) => {
                resp.set_retcode(1);
                resp.set_message(&e);
            }
        }
    } else {
        // absent: not implemented yet — would need rule number tracking
        resp.set_retcode(1);
        resp.set_message("--absent not yet implemented. Use --flush to clear all rules.");
    }

    resp
}
