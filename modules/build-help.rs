// Shared build-script helper for no_std modules.
// Include via `include!("../../.build-help.rs");` in your build.rs.
// Reads src/mod_doc.yaml and writes help.txt to OUT_DIR.

use std::fs;
use std::io::Write;

pub fn generate_help() {
    let yaml = fs::read_to_string("src/mod_doc.yaml").unwrap_or_default();
    let mut help = Vec::new();

    let mut name = "unknown";
    let mut version = "0.1.0";
    for line in yaml.lines() {
        let t = line.trim();
        if let Some(v) = t.strip_prefix("name:").map(|s| s.trim().trim_matches('"')) {
            name = v;
        }
        if let Some(v) = t.strip_prefix("version:").map(|s| s.trim().trim_matches('"')) {
            version = v;
        }
    }

    write!(help, "{name} v{version}\n\n").unwrap();

    let mut section = "";
    for line in yaml.lines() {
        let t = line.trim();
        if t == "options:" {
            section = "Options";
            continue;
        }
        if t == "arguments:" {
            section = "Arguments";
            continue;
        }
        if t == "examples:" || t == "returns:" {
            break;
        }
        if !section.is_empty() && t.starts_with("- name:") {
            let n = t.trim_start_matches("- name:").trim().trim_matches('"');
            if section == "Options" {
                writeln!(help, "  --{n}").unwrap();
            } else {
                writeln!(help, "  --{n} <value>").unwrap();
            }
        }
    }

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let path = format!("{out_dir}/help.txt");
    fs::File::create(&path).unwrap().write_all(&help).unwrap();

    println!("cargo:rustc-link-lib=c");
}
