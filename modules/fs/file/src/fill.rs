use libsysinspect::{
    cfg::mmconf::DEFAULT_FILESERVER_PORT,
    modlib::{
        response::ModResponse,
        runtime::{ArgValue, ModRequest},
    },
};
use reqwest::blocking::get;
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
fn pull(p: PathBuf, fileserver: String, src: String) -> Result<(), Error> {
    if p.exists() {
        return Err(Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("File {} already exists", p.to_str().unwrap_or_default()),
        ));
    }

    let url = format!("{}/{}", fileserver, src.strip_prefix("/").unwrap_or_default());
    let mut rs = match get(&url) {
        Ok(r) => r,
        Err(err) => return Err(Error::new(std::io::ErrorKind::ConnectionAborted, err)),
    };

    if rs.status().is_success() {
        let mut f = File::create(&p)?;
        std::io::copy(&mut rs, &mut f)?;
    } else {
        return Err(Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("Error accessing remote url at {}: {}", url, rs.status().canonical_reason().unwrap_or_default()),
        ));
    }

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
    if pn.is_empty() {
        rsp.set_message("Argument \"name\" is empty");
        rsp.set_retcode(1);
        return;
    }

    let fsr_addr = format!(
        "http://{}:{}",
        rq.config().get("master.ip").unwrap_or(&ArgValue::default()).as_string().unwrap_or_default(),
        rq.config()
            .get("master.fileserver.port")
            .unwrap_or(&ArgValue::default())
            .as_int()
            .unwrap_or(DEFAULT_FILESERVER_PORT.into())
    );

    if rq.args().contains_key("pull") {
        if let Err(err) = pull(
            PathBuf::from(&pn),
            fsr_addr,
            rq.args().get("pull").unwrap_or(&ArgValue::default()).as_string().unwrap_or_default(),
        ) {
            if strict {
                rsp.set_retcode(1);
            }
            rsp.set_message(&format!("Error pulling the file: {}", err));
            _ = rsp.cm_set_changed(false);

            return;
        }
    } else if let Err(err) = touch(PathBuf::from(&pn)) {
        if strict {
            rsp.set_retcode(1);
        }

        rsp.set_message(&format!("Touch error: {}", err));
        _ = rsp.cm_set_changed(false);

        return;
    }

    rsp.set_message(&format!("File {} created", pn));
    _ = rsp.cm_set_changed(true);
}
