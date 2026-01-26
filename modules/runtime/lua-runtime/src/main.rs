mod lrt;
use crate::lrt::{LuaRuntime, LuaRuntimeError};
use clap::Parser;
use libmodcore::{
    init_mod_doc,
    manrndr::print_mod_manual,
    modcli::ModuleCli,
    modinit::ModInterface,
    response::ModResponse,
    rtspec::RuntimeParams,
    runtime::{ModRequest, get_call_args, send_call_response},
};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Read Lua module code from file
fn read_module_code(modname: &str, scripts_dir: &Path) -> std::io::Result<String> {
    let path = scripts_dir.join(format!("{}.lua", modname));
    std::fs::read_to_string(path)
}

/// List available Lua modules in the scripts directory
fn list_lua_modules(scripts_dir: &Path) -> Vec<String> {
    let mut modules = Vec::new();

    if let Ok(entries) = std::fs::read_dir(scripts_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && let Some(ext) = path.extension()
                && ext == "lua"
                && let Some(stem) = path.file_stem()
                && let Some(stem_str) = stem.to_str()
            {
                modules.push(stem_str.to_string());
            }
        }
    }

    modules
}

/// Get module documentation from Lua runtime
fn module_doc_help(cli: &ModuleCli, modname: &str) -> Result<Value, LuaRuntimeError> {
    let rt = match LuaRuntime::new(PathBuf::from(cli.get_sharelib()), false) {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("Failed to create Lua runtime: {}", err);
            return Err(err);
        }
    };

    rt.module_doc(&read_module_code(modname, rt.get_scripts_dir()).unwrap_or_default())
}

/// Run the Lua runtime with the provided request.
fn call_runtime(cli: &ModuleCli, rq: &ModRequest) -> ModResponse {
    let mut resp = ModResponse::new_cm();

    // Get sharelib path from passed config or override from CLI or default
    let sharelib = rq.config().get("path.sharelib").and_then(|v| v.as_string()).unwrap_or(cli.get_sharelib());
    let rt = match LuaRuntime::new(PathBuf::from(&sharelib), rq.has_option(&format!("{}{}", RuntimeParams::RtPrefix, "native"))) {
        Ok(rt) => rt,
        Err(err) => {
            resp.set_message(&format!("Failed to create Lua runtime: {}", err));
            return resp;
        }
    };

    // list modules only?
    for opt in rq.options_all() {
        if opt.as_string().unwrap_or_default().eq(&format!("{}{}", RuntimeParams::RtPrefix, "list")) {
            let modules = list_lua_modules(rt.get_scripts_dir());
            match resp.set_data(serde_json::json!({ "modules": modules })) {
                Ok(_) => {}
                Err(err) => {
                    resp.set_message(&format!("Failed to set response data: {}", err));
                    return resp;
                }
            }
            resp.set_retcode(0);
            resp.set_message("Listed available Lua modules successfully.");
            return resp;
        }
    }

    // Call the module
    let modpath = match rq.args_all().get(&RuntimeParams::ModuleName.to_string()) {
        Some(v) => v.as_string().unwrap_or_default(),
        None => String::new(),
    };

    if modpath.is_empty() {
        let mut resp = ModResponse::new_cm();
        resp.set_message(&format!("No module name provided. Set '{}' argument properly.", RuntimeParams::ModuleName));
        return resp;
    }
    match rt.call_module(
        &modpath,
        &read_module_code(&modpath, rt.get_scripts_dir()).unwrap_or_default(),
        &serde_json::json!({"args": rq.args(), "config": rq.config(), "opts": rq.options(), "ext": rq.ext()}),
        rq.has_option(&format!("{}{}", RuntimeParams::RtPrefix, "logs")),
    ) {
        Ok(data) => {
            match resp.set_data(data) {
                Ok(_) => {
                    let _ = resp.cm_set_changed(true);
                }
                Err(err) => {
                    resp.set_message(&format!("Failed to set response data: {}", err));
                    return resp;
                }
            }
            resp.set_retcode(0);
            resp.set_message("Called Lua module successfully.");
        }
        Err(err) => {
            resp.set_message(&format!("Failed to execute Lua code: {}. Scripts directory: {}", err, rt.get_scripts_dir().display()));
            return resp;
        }
    };

    resp
}

/// Main entry point
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
                print_mod_manual(&doc);
            }
            Err(err) => {
                eprintln!("Failed to get module documentation: {}", err);
            }
        }
        return;
    } else if cli.is_list_modules() {
        println!("Available Lua runtime modules:");
        for module in list_lua_modules(PathBuf::from(cli.get_sharelib()).as_path()) {
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
