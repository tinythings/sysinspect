// ---------------------------------------------------------------------------
// net.ipfw — cross-platform firewall management (v0.1.0)
//
// Architecture: one backend per OS, auto-detected at runtime.
// Common rule expressions are translated to backend-specific commands.
// Users can also pass raw "native" rules for their backend.
// ---------------------------------------------------------------------------

use libmodcore::{
    response::ModResponse,
    runtime::{self, ModRequest},
};
use std::process::Command;

// ---------------------------------------------------------------------------
// Common rule expression (proposal)
// ---------------------------------------------------------------------------
// Fields are optional unless noted.
// {
//   "action":      "allow" | "deny",                            // required for mutation
//   "protocol":    "tcp" | "udp" | "icmp" | "any",              // default: tcp
//   "source":      "192.168.1.0/24" | "any",                    // default: any
//   "destination": "10.0.0.1" | "any",                          // default: any
//   "port":        "80" | "443",                                // single port
//   "port-range":  "8000-8080",                                 // range
//   "interface":   "eth0" | "em0",                              // network iface
//   "direction":   "in" | "out",                                // default: in
//   "stateful":    true | false,                                // keep state (where supported)
//   "log":         true | false,                                // log matching packets
//   "comment":     "Allow web traffic",                         // rule comment
//   "native": {                                                 // raw backend rules
//     "pf":   "pass in proto tcp from any to any port 80",
//     "ipfw": "add allow tcp from any to any 80 in",
//     "nft":  "add rule inet filter input tcp dport 80 accept",
//     "iptables": "-A INPUT -p tcp --dport 80 -j ACCEPT"
//   }
// }
// ---------------------------------------------------------------------------

/// Detect the active firewall backend on this host.
pub(crate) fn detect_backend() -> Option<&'static str> {
    let candidates: &[(&str, &[&str])] = &[
        ("pf", &["pfctl", "-s", "info"]),
        ("ipfw", &["ipfw", "list"]),
        ("nftables", &["nft", "list", "ruleset"]),
        ("iptables", &["iptables", "-L", "-n"]),
    ];
    candidates.iter().find_map(|(name, args)| Command::new(args[0]).args(&args[1..]).output().ok().filter(|o| o.status.success()).map(|_| *name))
}

/// Translate a common rule expression into backend-specific command.
fn translate_rule(backend: &str, args: &serde_json::Value) -> Result<Vec<String>, String> {
    let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("allow");
    let proto = args.get("protocol").and_then(|v| v.as_str()).unwrap_or("tcp");
    let port = args.get("port").and_then(|v| v.as_str()).unwrap_or("");
    let src = args.get("source").and_then(|v| v.as_str()).unwrap_or("any");
    let dst = args.get("destination").and_then(|v| v.as_str()).unwrap_or("any");
    let iface = args.get("interface").and_then(|v| v.as_str()).unwrap_or("");
    let dir = args.get("direction").and_then(|v| v.as_str()).unwrap_or("in");

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
                rule.push_str(&format!(" proto {}", proto));
            }
            if src != "any" {
                rule.push_str(&format!(" from {}", src));
            } else {
                rule.push_str(" from any");
            }
            if dst != "any" {
                rule.push_str(&format!(" to {}", dst));
            } else {
                rule.push_str(" to any");
            }
            if !port.is_empty() {
                rule.push_str(&format!(" port {}", port));
            }
            if !iface.is_empty() {
                rule.push_str(&format!(" on {}", iface));
            }
            Ok(vec!["pfctl".to_string(), "-o".to_string(), rule])
        }
        "ipfw" => {
            let act = if action == "allow" { "allow" } else { "deny" };
            let mut rule = format!("add {} {}", act, proto);
            if src != "any" {
                rule.push_str(&format!(" from {}", src));
            } else {
                rule.push_str(" from any");
            }
            if dst != "any" {
                rule.push_str(&format!(" to {}", dst));
            } else {
                rule.push_str(" to any");
            }
            if !port.is_empty() {
                rule.push_str(&format!(" dst-port {}", port));
            }
            if !iface.is_empty() {
                rule.push_str(&format!(" via {}", iface));
            }
            if dir == "out" {
                rule.push_str(" out");
            } else {
                rule.push_str(" in");
            }
            Ok(vec!["ipfw".to_string(), "-q".to_string(), rule])
        }
        "nftables" => {
            let act = if action == "allow" { "accept" } else { "drop" };
            let mut rule = format!("add rule inet filter");
            if dir == "out" {
                rule.push_str(" output");
            } else {
                rule.push_str(" input");
            }
            if proto != "any" {
                rule.push_str(&format!(" {} dport {}", proto, port));
            }
            if src != "any" {
                rule.push_str(&format!(" ip saddr {}", src));
            }
            if dst != "any" {
                rule.push_str(&format!(" ip daddr {}", dst));
            }
            rule.push_str(&format!(" {}", act));
            Ok(vec!["nft".to_string(), rule])
        }
        "ipfw" => {
            let act = if action == "allow" { "allow" } else { "deny" };
            let mut rule = format!("add {} {}", act, proto);
            if src != "any" {
                rule.push_str(&format!(" from {}", src));
            } else {
                rule.push_str(" from any");
            }
            if dst != "any" {
                rule.push_str(&format!(" to {}", dst));
            } else {
                rule.push_str(" to any");
            }
            if !port.is_empty() {
                rule.push_str(&format!(" dst-port {}", port));
            }
            if !iface.is_empty() {
                rule.push_str(&format!(" via {}", iface));
            }
            if dir == "out" {
                rule.push_str(" out");
            } else {
                rule.push_str(" in");
            }
            Ok(vec!["ipfw".to_string(), "-q".to_string(), rule])
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
                rule.push_str(&format!(" {} dport {}", proto, port));
            }
            if src != "any" {
                rule.push_str(&format!(" ip saddr {}", src));
            }
            if dst != "any" {
                rule.push_str(&format!(" ip daddr {}", dst));
            }
            rule.push_str(&format!(" {}", act));
            Ok(vec!["nft".to_string(), rule])
        }
        "iptables" => {
            let chain = if dir == "out" { "OUTPUT" } else { "INPUT" };
            let jump = if action == "allow" { "ACCEPT" } else { "DROP" };
            let mut args = vec!["-A".to_string(), chain.to_string(), "-p".to_string(), proto.to_string()];
            if !port.is_empty() {
                args.push("--dport".to_string());
                args.push(port.to_string());
            }
            if src != "any" {
                args.push("-s".to_string());
                args.push(src.to_string());
            }
            if dst != "any" {
                args.push("-d".to_string());
                args.push(dst.to_string());
            }
            if !iface.is_empty() {
                args.push("-i".to_string());
                args.push(iface.to_string());
            }
            args.push("-j".to_string());
            args.push(jump.to_string());
            Ok(args)
        }
        _ => Err(format!("Unsupported backend: {}", backend)),
    }
}

pub fn run(rt: &ModRequest) -> ModResponse {
    let mut resp = runtime::new_call_response();

    let backend = detect_backend();
    if backend.is_none() {
        resp.set_retcode(1);
        resp.set_message("No supported firewall backend detected (tried: pf, ipfw, nftables, iptables)");
        return resp;
    }
    resp.add_warning(&format!("Detected backend: {}", backend.unwrap()));

    // TODO: wire opts/args parsing, call translate_rule, execute
    resp.set_retcode(0);
    resp.set_message("net.ipfw scaffold — backend detection works, rule translation TBD");
    resp
}
