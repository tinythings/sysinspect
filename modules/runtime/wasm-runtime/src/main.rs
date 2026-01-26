mod wart;

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
use std::path::{Path, PathBuf};

/// List available Wasm modules in the scripts directory
fn list_wasm_modules(_wasm_dir: &Path) -> Vec<String> {
    Vec::new()
}

/// Get module documentation from Wasm runtime
fn module_doc_help(_cli: &ModuleCli, _modname: &str) -> Result<Value, SysinspectError> {
    Ok(json!({}))
}

/// Run the Wasm runtime with the provided request.
fn call_runtime(_cli: &ModuleCli, rq: &ModRequest) -> ModResponse {
    let mut r = ModResponse::new_cm();
    let rt = match wart::WasmRuntime::new(rq) {
        Err(err) => {
            r.set_message(&format!("Failed to initialize Wasm runtime: {err}"));
            r.set_retcode(4);
            return r;
        }
        Ok(rt) => rt,
    };
    

    rt.run()
}

fn main() {
    let mod_doc = init_mod_doc!(ModInterface);
    let cli = ModuleCli::parse();

    // CLI calls from the terminal directly
    if cli.is_manual() {
        print!("{}", mod_doc.help());
        return;
    } else if !cli.get_help_on().is_empty() {
        match get_call_args() {
            Ok(mut rq) => {
                rq.add_opt("man");
                rq.add_arg("man", Value::Bool(true));
                let mr = &call_runtime(&cli, &rq);
                print_mod_manual(mr.get_data());
            }
            Err(err) => println!("Arguments error: {err}"),
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
