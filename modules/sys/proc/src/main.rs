use libsysinspect::modlib::{
    modinit::ModInterface,
    runtime::{self},
};
use sysproc::run;
mod sysproc;

fn main() {
    let mod_doc = libsysinspect::init_mod_doc!(ModInterface);
    if mod_doc.print_help() {
        return;
    }

    match runtime::get_call_args() {
        Ok(rt) => match runtime::send_call_response(&run(&rt)) {
            Ok(_) => {}
            Err(err) => {
                println!("Error: {}", err)
            }
        },
        Err(err) => {
            println!("{}", err);
        }
    }
}
