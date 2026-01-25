use std::path::{Path, PathBuf};

use clap::Parser;
use libmodcore::{
    init_mod_doc,
    manrndr::print_mod_manual,
    modcli::ModuleCli,
    modinit::ModInterface,
    response::ModResponse,
    runtime::{ModRequest, get_call_args, send_call_response},
};
use libsysinspect::SysinspectError;
use serde_json::{Value, json};

/// List available Wasm modules in the scripts directory
fn list_wasm_modules(_wasm_dir: &Path) -> Vec<String> {
    Vec::new()
}

/// Get module documentation from Wasm runtime
fn module_doc_help(_cli: &ModuleCli, _modname: &str) -> Result<Value, SysinspectError> {
    Ok(json!({}))
}

/// Run the Wasm runtime with the provided request.
fn call_runtime(_cli: &ModuleCli, _rq: &ModRequest) -> ModResponse {
    ModResponse::default()
}

fn main() {
    let mod_doc = init_mod_doc!(ModInterface);
    let cli = ModuleCli::parse();

    // CLI calls from the terminal directly
    if cli.is_manual() {
        print!("{}", mod_doc.help());
        return;
    } else if !cli.get_help_on().is_empty() {
        match module_doc_help(&cli, &cli.get_help_on()) {
            Ok(doc) => {
                print_mod_manual(doc);
            }
            Err(err) => {
                eprintln!("Failed to get module documentation: {}", err);
            }
        }
        return;
    } else if cli.is_list_modules() {
        println!("Available Wasm runtime modules:");
        for module in list_wasm_modules(PathBuf::from(cli.get_sharelib()).as_path()) {
            println!("  - {}", module);
        }
        return;
    }

    // Runtime call (integrated via JSON protocol)
    match get_call_args() {
        Ok(rq) => match send_call_response(&call_runtime(&cli, &rq)) {
            Ok(_) => {}
            Err(err) => println!("Runtime error: {err}"),
        },
        Err(err) => println!("Arguments error: {err}"),
    }
}
