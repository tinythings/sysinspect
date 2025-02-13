mod fdel;
mod fill;

use libsysinspect::{
    init_mod_doc,
    modlib::{
        modinit::ModInterface,
        response::ModResponse,
        runtime::{get_call_args, send_call_response, ArgValue, ModRequest},
    },
};

/// Run module
fn run_mod(rq: &ModRequest) -> ModResponse {
    let mut resp = ModResponse::new_cm();

    if rq.options().len() != 1 {
        resp.set_message(&format!(
            "This module requires only one option. {} has been given.",
            if rq.options().len() > 1 { "Multiple" } else { "None" }
        ));
        return resp;
    }

    let strict = rq.args().get("mode").unwrap_or(&ArgValue::default()).as_string().unwrap_or_default().eq("strict");

    if !rq.args().contains_key("name") {
        resp.set_message("Argument \"name\" is required");
        return resp;
    }

    if rq.args().get("name").unwrap_or(&ArgValue::default()).as_string().unwrap_or_default().is_empty() {
        resp.set_message("Argument \"name\" is empty");
        return resp;
    }

    match rq.options().first().unwrap_or(&ArgValue::default()).as_string().unwrap_or_default().as_str() {
        "create" => fill::do_create(rq, &mut resp, strict),
        "delete" => fdel::do_delete(rq, &mut resp, strict),
        opt => {
            resp.set_message(&format!("Unknown option: {}", opt));
            return resp;
        }
    }

    resp
}

/// Init module
fn main() {
    let mod_doc = init_mod_doc!(ModInterface);
    if mod_doc.print_help() {
        return;
    }

    match get_call_args() {
        Ok(rq) => match send_call_response(&run_mod(&rq)) {
            Ok(_) => {}
            Err(err) => println!("Runtime error: {}", err),
        },
        Err(err) => {
            println!("Arguments error: {}", err)
        }
    }
}
