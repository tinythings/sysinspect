use serde_json::json;
use std::fs;

fn main() {
    let path = "/etc/machine-id";
    match fs::read_to_string(path) {
        Ok(contents) => {
            let id = contents.trim().to_string();
            println!("{}", json!({ "minion_id": id }).to_string());
        }
        Err(_) => {
            println!("{}", json!({ "error": "Could not read machine-id file" }).to_string());
        }
    }
}
