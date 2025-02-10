use libsysinspect::{init_mod_doc, modlib::modinit::ModInterface};
fn main() {
    let mod_doc = init_mod_doc!(ModInterface);
    if mod_doc.print_help() {
        return;
    }
}
