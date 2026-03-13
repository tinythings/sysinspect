use serde_json::json;
use std::fs;

mod sysinspect_host {
    include!("../../rust-sdk/host.rs");
}

fn main() {
    let path = "/etc/machine-id";
    match fs::read_to_string(path) {
        Ok(contents) => {
            let id = contents.trim().to_string();
            println!(
                "{}",
                json!({
                    "minion_id": id,
                    "hostname": sysinspect_host::trait_value("system.hostname"),
                    "sharelib": sysinspect_host::path_value("sharelib")
                })
            );
        }
        Err(_) => {
            println!(
                "{}",
                json!({
                    "error": "Could not read machine-id file",
                    "hostname": sysinspect_host::trait_value("system.hostname"),
                    "sharelib": sysinspect_host::path_value("sharelib")
                })
            );
        }
    }
}
