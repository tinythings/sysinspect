mod fcp;
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
    let mut resp = ModResponse::new();

    if rq.options().len() != 1 {
        resp.set_message(&format!(
            "This module requires only one option. {} has been given.",
            if rq.options().len() > 1 { "Multiple" } else { "None" }
        ));
        resp.set_retcode(1);
        return resp;
    }

    let strict = rq.args().get("mode").unwrap_or(&ArgValue::default()).as_string().unwrap_or_default().eq("strict");

    match rq.options().first().unwrap_or(&ArgValue::default()).as_string().unwrap_or_default().as_str() {
        "fill" => fill::do_fill(rq, &mut resp, strict),
        "delete" => fdel::do_delete(rq, &mut resp, strict),
        "copy" => fcp::do_copy(rq, &mut resp, strict),
        opt => {
            resp.set_message(&format!("Unknown option: {}", opt));
            resp.set_retcode(1);
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
