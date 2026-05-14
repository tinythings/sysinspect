use libmodcore::{
    response::ModResponse,
    runtime::{self, ModRequest},
};
use std::collections::HashMap;
use std::process::Command;

/// ICMP ping a host. Parses ping output cross-platform (Linux, BSD, macOS).
/// Returns telemetry: host, sent, received, loss_pct, rtt_min, rtt_avg, rtt_max, ttl.
pub fn ping_host(rt: &ModRequest, rsp: &mut ModResponse) {
    let host = runtime::get_arg(rt, "host");
    let count = runtime::get_arg(rt, "count");
    let timeout = runtime::get_arg(rt, "timeout");

    if host.is_empty() {
        rsp.set_retcode(1);
        rsp.set_message("Argument \"host\" is required for --ping");
        return;
    }

    let n = count.parse::<u32>().unwrap_or(3);
    let t = timeout.parse::<u32>().unwrap_or(3);

    let (success, parsed) = run_ping(&host, n, t);

    let mut data = HashMap::new();
    data.insert("host".to_string(), serde_json::Value::String(host.clone()));

    match parsed {
        Some(stats) => {
            data.insert("sent".to_string(), serde_json::Value::Number(serde_json::Number::from(stats.sent)));
            data.insert("received".to_string(), serde_json::Value::Number(serde_json::Number::from(stats.received)));
            data.insert(
                "loss_pct".to_string(),
                serde_json::Value::Number(serde_json::Number::from_f64(stats.loss_pct).unwrap_or(serde_json::Number::from(0))),
            );
            if let Some(v) = stats.rtt_min {
                data.insert("rtt_min".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(v).unwrap_or(serde_json::Number::from(0))));
            }
            if let Some(v) = stats.rtt_avg {
                data.insert("rtt_avg".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(v).unwrap_or(serde_json::Number::from(0))));
            }
            if let Some(v) = stats.rtt_max {
                data.insert("rtt_max".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(v).unwrap_or(serde_json::Number::from(0))));
            }
            if let Some(v) = stats.ttl {
                data.insert("ttl".to_string(), serde_json::Value::Number(serde_json::Number::from(v)));
            }

            rsp.set_retcode(if success { 0 } else { 1 });
            rsp.set_message(&format!("Ping to {host}: {} sent, {} received, {:.1}% loss", stats.sent, stats.received, stats.loss_pct));
        }
        None => {
            data.insert("sent".to_string(), serde_json::Value::Number(serde_json::Number::from(n)));
            data.insert("received".to_string(), serde_json::Value::Number(serde_json::Number::from(0)));
            data.insert("loss_pct".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(100.0).unwrap()));

            rsp.set_retcode(1);
            rsp.set_message(&format!("Ping to {host} failed or timed out"));
        }
    }

    if let Err(e) = rsp.set_data(&data) {
        rsp.add_warning(&format!("{e}"));
    }
}

pub(crate) struct PingStats {
    pub(crate) sent: u32,
    pub(crate) received: u32,
    pub(crate) loss_pct: f64,
    pub(crate) rtt_min: Option<f64>,
    pub(crate) rtt_avg: Option<f64>,
    pub(crate) rtt_max: Option<f64>,
    pub(crate) ttl: Option<u32>,
}

fn run_ping(host: &str, count: u32, timeout: u32) -> (bool, Option<PingStats>) {
    let args = ping_args(count, timeout, host);
    let output = match Command::new("ping").args(&args).output() {
        Ok(o) => o,
        Err(_) => return (false, None),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");
    let success = output.status.success();

    let stats = parse_ping_output(&combined, count);
    (success, stats)
}

fn ping_args(count: u32, timeout: u32, host: &str) -> Vec<String> {
    if cfg!(target_os = "linux") || cfg!(target_os = "android") {
        vec!["-c".to_string(), count.to_string(), "-W".to_string(), timeout.to_string(), host.to_string()]
    } else {
        vec!["-c".to_string(), count.to_string(), "-t".to_string(), timeout.to_string(), host.to_string()]
    }
}

pub(crate) fn parse_ping_output(output: &str, sent: u32) -> Option<PingStats> {
    let mut received = 0u32;
    let mut loss_pct = 100.0f64;
    let mut rtt_min = None;
    let mut rtt_avg = None;
    let mut rtt_max = None;
    let mut ttl = None;

    for line in output.lines() {
        let t = line.trim();

        if (t.contains("packets transmitted") || t.contains("packet loss") || t.contains("statistics"))
            && let Some(r) = parse_sent_received(t, sent)
        {
            received = r;
            if sent > 0 {
                loss_pct = ((sent - received) as f64 / sent as f64) * 100.0;
            }
        }

        if t.contains("min/avg/max") || t.contains("rtt min/avg/max") || t.contains("round-trip") {
            let (min, avg, max) = parse_rtt(t);
            rtt_min = min;
            rtt_avg = avg;
            rtt_max = max;
        }

        if t.contains("ttl=") {
            ttl = parse_ttl(t);
        }
    }

    Some(PingStats { sent, received, loss_pct, rtt_min, rtt_avg, rtt_max, ttl })
}

pub(crate) fn parse_sent_received(line: &str, _fallback_sent: u32) -> Option<u32> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    for (i, t) in tokens.iter().enumerate() {
        if *t == "received," && i > 0 {
            return tokens[i - 1].parse().ok();
        }
    }
    None
}

pub(crate) fn parse_rtt(line: &str) -> (Option<f64>, Option<f64>, Option<f64>) {
    let parts: Vec<&str> = line.split('=').collect();
    if parts.len() < 2 {
        return (None, None, None);
    }
    let vals: Vec<f64> = parts[1].split('/').filter_map(|s| s.trim().parse().ok()).collect();
    if vals.len() >= 3 { (Some(vals[0]), Some(vals[1]), Some(vals[2])) } else { (None, None, None) }
}

pub(crate) fn parse_ttl(line: &str) -> Option<u32> {
    line.split("ttl=").nth(1).and_then(|s| s.split_whitespace().next()).and_then(|s| s.parse().ok())
}
