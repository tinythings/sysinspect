mod wart;

use clap::Parser;
use colored::Colorize;
use libmodcore::{
    init_mod_doc,
    manrndr::print_mod_manual,
    modcli::ModuleCli,
    modinit::ModInterface,
    response::ModResponse,
    runtime::{ModRequest, get_call_args, send_call_response},
};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// List available Wasm modules in the scripts directory
fn list_wasm_modules(wasm_dir: &Path) {
    let rt = match wart::WasmRuntime::new(&ModRequest::default()) {
        Err(err) => {
            println!("Failed to initialize Wasm runtime: {err}");
            return;
        }
        Ok(rt) => rt,
    };
    let mut mods = match rt.get_wasm_modules() {
        Ok(mods) => mods,
        Err(err) => {
            println!("Failed to list Wasm modules in {}: {err}", wasm_dir.display());
            return;
        }
    };
    mods.sort();

    println!("Available Wasm/WASI modules:");
    for (i, m) in mods.iter().enumerate() {
        println!(" {}. {}", i + 1, m.bright_green());
    }
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
        let mut rq = ModRequest::default();
        rq.add_opt("man");
        rq.add_arg("rt.mod", Value::String(cli.get_help_on()));
        print_mod_manual(call_runtime(&cli, &rq).get_data());
        return;
    } else if cli.is_list_modules() {
        list_wasm_modules(PathBuf::from(cli.get_sharelib()).as_path());
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
