mod docschema;
mod lrt;
use crate::lrt::LuaRuntime;
use libmodcore::{
    init_mod_doc,
    modinit::ModInterface,
    response::ModResponse,
    runtime::{ModRequest, get_call_args, send_call_response},
};
use serde_json::json;

fn read_module_code(modname: &str) -> std::io::Result<String> {
    let path = format!("./{}.lua", modname);
    std::fs::read_to_string(path)
}

/// Run the Lua runtime with the provided request.
fn call_runtime(rq: &ModRequest) -> ModResponse {
    let modpath = match rq.args().get("mod") {
        Some(v) => v.as_string().unwrap_or_default(),
        None => String::new(),
    };

    if modpath.is_empty() {
        let mut resp = ModResponse::new_cm();
        resp.set_message("No module name provided. Set 'mod' argument properly.");
        return resp;
    }

    let mut resp = ModResponse::new_cm();
    let rt = match LuaRuntime::new() {
        Ok(rt) => rt,
        Err(err) => {
            resp.set_message(&format!("Failed to create Lua runtime: {}", err));
            return resp;
        }
    };

    // Return module documentation
    if rq.args().contains_key("man") {
        match rt.module_doc(&read_module_code(&modpath).unwrap_or_default()) {
            Ok(doc) => {
                match resp.set_data(json!({ "manpage": doc })) {
                    Ok(_) => {
                        let _ = resp.cm_set_changed(false);
                    }
                    Err(err) => {
                        resp.set_message(&format!("Failed to set response data: {}", err));
                        return resp;
                    }
                }
                resp.set_retcode(0);
                resp.set_message("Module documentation retrieved successfully.");
            }
            Err(err) => {
                resp.set_message(&format!("Failed to get module documentation: {}", err));
                return resp;
            }
        };
        return resp;
    }

    // Call the module
    match rt.call_module(
        &read_module_code(&modpath).unwrap_or_default(),
        &serde_json::json!({"args": rq.args(), "config": rq.config(), "opts": rq.options(), "ext": rq.ext()}),
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
            resp.set_message(&format!("Failed to execute Lua code: {}", err));
            return resp;
        }
    };

    resp
}

fn main() {
    let mod_doc = init_mod_doc!(ModInterface);
    if mod_doc.print_help() {
        return;
    }

    match get_call_args() {
        Ok(rq) => match send_call_response(&call_runtime(&rq)) {
            Ok(_) => {}
            Err(err) => println!("Runtime error: {err}"),
        },
        Err(err) => println!("Arguments error: {err}"),
    }
}
