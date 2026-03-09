mod prt;

use crate::prt::{Py3Runtime, Py3RuntimeError};
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

/// List available Python runtime modules
/// # Arguments
/// * `scripts_dir` - Python runtime scripts directory
/// # Returns
/// * `Vec<String>` - Available module names
fn list_python_modules(scripts_dir: &Path) -> Vec<String> {
    let mut modules = Vec::new();
    fn visit(root: &Path, dir: &Path, out: &mut Vec<String>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    visit(root, &path, out);
                } else if path.is_file()
                    && let Some(ext) = path.extension()
                    && ext == "py"
                    && let Ok(rel) = path.strip_prefix(root)
                {
                    let ns = rel.to_string_lossy().trim_end_matches(".py").replace('/', ".").replace('\\', ".");
                    if !ns.is_empty() && !ns.ends_with(".__init__") {
                        out.push(ns);
                    }
                }
            }
        }
    }

    visit(scripts_dir, scripts_dir, &mut modules);
    modules.sort();
    modules
}

/// Get manual documentation for a Python runtime module
/// # Arguments
/// * `cli` - Parsed module CLI
/// * `modname` - Runtime module name
/// # Returns
/// * `Result<Value, Py3RuntimeError>` - Runtime module documentation
fn module_doc_help(cli: &ModuleCli, modname: &str) -> Result<Value, Py3RuntimeError> {
    let rt = Py3Runtime::new(PathBuf::from(cli.get_sharelib()))?;
    rt.module_doc(&rt.read_module_code(modname)?)
}

/// Run the Python runtime with the provided request
/// # Arguments
/// * `cli` - Parsed module CLI
/// * `rq` - Runtime request
/// # Returns
/// * `ModResponse` - Runtime call response
fn call_runtime(cli: &ModuleCli, rq: &ModRequest) -> ModResponse {
    let mut resp = ModResponse::new_cm();
    let sharelib = rq.config().get("path.sharelib").and_then(|v| v.as_string()).unwrap_or(cli.get_sharelib());
    let rt = match Py3Runtime::new(PathBuf::from(&sharelib)) {
        Ok(rt) => rt,
        Err(err) => {
            resp.set_message(&format!("Failed to create Python runtime: {err}"));
            return resp;
        }
    };

    for opt in rq.options_all() {
        if opt.as_string().unwrap_or_default().eq(&format!("{}{}", RuntimeParams::RtPrefix, "list")) {
            match resp.set_data(serde_json::json!({ "modules": list_python_modules(rt.get_scripts_dir()) })) {
                Ok(_) => {
                    resp.set_retcode(0);
                    resp.set_message("Listed available Python modules successfully.");
                }
                Err(err) => resp.set_message(&format!("Failed to set response data: {err}")),
            }
            return resp;
        }
    }

    let modpath = match rq.args_all().get(&RuntimeParams::ModuleName.to_string()) {
        Some(v) => v.as_string().unwrap_or_default(),
        None => String::new(),
    };
    if modpath.is_empty() {
        resp.set_message(&format!("No module name provided. Set '{}' argument properly.", RuntimeParams::ModuleName));
        return resp;
    }

    if rq.args_all().get(&RuntimeParams::ModuleManual.to_string()).and_then(|v| v.as_bool()).unwrap_or(false) {
        match rt.module_doc(&match rt.read_module_code(&modpath) {
            Ok(code) => code,
            Err(err) => {
                resp.set_message(&format!("Failed to read Python module: {err}"));
                return resp;
            }
        }) {
            Ok(data) => match resp.set_data(data) {
                Ok(_) => {
                    resp.set_retcode(0);
                    resp.set_message("Got Python module documentation successfully.");
                }
                Err(err) => resp.set_message(&format!("Failed to set response data: {err}")),
            },
            Err(err) => resp.set_message(&format!("Failed to get Python module documentation: {err}")),
        }
        return resp;
    }

    match rt.call_module(
        &modpath,
        &match rt.read_module_code(&modpath) {
            Ok(code) => code,
            Err(err) => {
                resp.set_message(&format!("Failed to read Python module: {err}"));
                return resp;
            }
        },
        &serde_json::json!({"args": rq.args(), "config": rq.config(), "opts": rq.options(), "ext": rq.ext()}),
        rq.has_option(&format!("{}{}", RuntimeParams::RtPrefix, "logs")),
    ) {
        Ok(data) => {
            match resp.set_data(data) {
                Ok(_) => {
                    let _ = resp.cm_set_changed(true);
                    resp.set_retcode(0);
                    resp.set_message("Called Python module successfully.");
                }
                Err(err) => resp.set_message(&format!("Failed to set response data: {err}")),
            }
        }
        Err(err) => resp.set_message(&format!(
            "Failed to execute Python code: {}. Scripts directory: {}",
            err,
            rt.get_scripts_dir().display()
        )),
    }

    resp
}

/// Main entry point
fn main() {
    let mod_doc = init_mod_doc!(ModInterface);
    let cli = ModuleCli::parse();

    if cli.is_manual() {
        print!("{}", mod_doc.help());
        return;
    } else if !cli.get_help_on().is_empty() {
        match module_doc_help(&cli, &cli.get_help_on()) {
            Ok(doc) => print_mod_manual(&doc),
            Err(err) => eprintln!("Failed to get module documentation: {err}"),
        }
        return;
    } else if cli.is_list_modules() {
        println!("Available Python runtime modules:");
        for module in list_python_modules(PathBuf::from(cli.get_sharelib()).join("lib/runtime/python3").as_path()) {
            println!("  - {}", module);
        }
        return;
    }

    match get_call_args() {
        Ok(rq) => match send_call_response(&call_runtime(&cli, &rq)) {
            Ok(_) => {}
            Err(err) => println!("Runtime error: {err}"),
        },
        Err(err) => println!("Arguments error: {err}"),
    }
}
