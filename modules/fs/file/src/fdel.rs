use libsysinspect::modlib::{
    response::ModResponse,
    runtime::{ArgValue, ModRequest},
};
use std::path::PathBuf;

/// Do file delete
pub fn do_delete(rq: &ModRequest, rsp: &mut ModResponse, strict: bool) {
    let pn = PathBuf::from(rq.args().get("name").unwrap_or(&ArgValue::default()).as_string().unwrap_or_default());
    if pn.exists() {
        if let Err(err) = std::fs::remove_file(&pn) {
            if strict {
                rsp.set_retcode(1);
            }
            rsp.set_message(&format!("Error deleting file \"{}\": {}", pn.to_str().unwrap_or_default(), err));
            _ = rsp.cm_set_changed(false);

            return;
        }
    }

    rsp.set_message(&format!("File \"{}\" was deleted", pn.to_str().unwrap_or_default()));
    _ = rsp.cm_set_changed(true);
}
