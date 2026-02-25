use std::collections::HashMap;

use clap::Parser;
use libmodcore::{
    init_mod_doc,
    modcli::ModuleCli,
    modinit::ModInterface,
    modlogger::{init_module_logger, take_logs},
    response::ModResponse,
    rtspec::RuntimeSpec,
    runtime::{ModRequest, get_call_args, send_call_response},
};
use serde_json::json;

/// Main module logic
fn run(cli: &ModuleCli, rq: &ModRequest) -> ModResponse {
    let mut data: HashMap<String, serde_json::Value> = HashMap::new();
    let mut resp = ModResponse::new_cm();
    if rq.has_option("push") && rq.has_option("pull") {
        resp.set_message("Configuration error: cannot have both push and pull options");
        return resp;
    }

    if rq.has_option("push") {
        log::info!("Push option selected. Resource will be uploaded to the storage.");
    } else if rq.has_option("pull") {
        log::info!("Pull option selected. Resource will be downloaded from the storage.");
    } else {
        log::error!("No valid option selected. Must have either push or pull.");
        resp.set_message("Configuration error: must have either push or pull option");
    }

    data.insert(RuntimeSpec::LogsSectionField.to_string(), json!(take_logs()));
    _ = resp.set_data(data);
    resp
}

fn main() {
    let mod_doc = init_mod_doc!(ModInterface);
    init_module_logger(mod_doc.name());

    let cli = ModuleCli::parse();

    // CLI calls from the terminal directly
    if cli.is_manual() {
        print!("{}", mod_doc.help());
        return;
    }

    // Runtime call (integrated via JSON protocol)
    match get_call_args() {
        Ok(rq) => match send_call_response(&run(&cli, &rq)) {
            Ok(_) => {}
            Err(err) => println!("Runtime error: {err}"),
        },
        Err(err) => println!("Arguments error: {err}"),
    }
}
