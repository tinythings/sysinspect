use libmodcore::{init_mod_doc, modinit::ModInterface, runtime};

mod http;
mod http_ut;

use http::HttpModule;

fn main() {
    let mod_doc = init_mod_doc!(ModInterface);
    if mod_doc.print_help() {
        return;
    }

    match runtime::get_call_args() {
        Ok(rt) => match runtime::send_call_response(&HttpModule::new(&rt).run()) {
            Ok(_) => {}
            Err(err) => println!("Module runtime error: {err}"),
        },
        Err(err) => println!("Arguments error: {err}"),
    }
}
