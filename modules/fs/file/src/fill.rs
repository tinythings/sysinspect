use libsysinspect::modlib::{
    response::ModResponse,
    runtime::{ArgValue, ModRequest},
};
use std::{fs::File, io::Error, path::PathBuf};

/// Create an empty file
fn touch(p: PathBuf) -> Result<(), Error> {
    if p.exists() {
        return Err(Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("File {} already exists", p.to_str().unwrap_or_default()),
        ));
    }

    File::create(p)?;

    Ok(())
}

/// Fill-in the file with the content from the fileserver
fn fill(p: PathBuf) -> Result<(), Error> {
    Ok(())
}

/// Do file filling
pub fn do_fill(rq: &ModRequest, rsp: &mut ModResponse, strict: bool) {
    if let Err(error) = rsp.cm_set_changed(true) {
        rsp.set_message(&format!("Data error: {}", error));
        rsp.set_retcode(255);
        return;
    }

    if !rq.args().contains_key("name") {
        rsp.set_message("Argument \"name\" is required");
        rsp.set_retcode(1);
        return;
    }

    let pn = rq.args().get("name").unwrap_or(&ArgValue::default()).as_string().unwrap_or_default();

    if rq.args().contains_key("pull") {
        // XXX: download stuff
    } else {
        if pn.is_empty() {
            rsp.set_message("Argument \"name\" is empty");
            rsp.set_retcode(1);
            return;
        }

        if let Err(err) = touch(PathBuf::from(&pn)) {
            if strict {
                rsp.set_retcode(1);
            }

            rsp.set_message(&format!("Touch error: {}", err));
            _ = rsp.cm_set_changed(false);

            return;
        }
    }

    rsp.set_message(&format!("File {} created", pn));
    _ = rsp.cm_set_changed(true);
}
