use libmodcore::{
    response::ModResponse,
    runtime::{ArgValue, ModRequest},
};
use libsysinspect::cfg::mmconf::DEFAULT_FILESERVER_PORT;
use reqwest::blocking::get;
use std::{
    fs::{self, File},
    io::Error,
    path::PathBuf,
};

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
fn download(p: PathBuf, fileserver: String, src: String) -> Result<(), Error> {
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

fn copy(src: PathBuf, dst: PathBuf) -> Result<(), Error> {
    if !src.exists() {
        return Err(Error::new(
            std::io::ErrorKind::NotFound,
            format!("Error copying file: source file {} was not found", src.to_str().unwrap_or_default()),
        ));
    } else if dst.exists() {
        return Err(Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("Error copying file: destination file {} already exists", dst.to_str().unwrap_or_default()),
        ));
    }

    fs::copy(src, dst)?;

    Ok(())
}

/// Do file filling
pub fn do_create(rq: &ModRequest, rsp: &mut ModResponse, strict: bool) {
    rsp.set_retcode(0);

    if let Err(error) = rsp.cm_set_changed(true) {
        rsp.set_message(&format!("Data error: {}", error));
        rsp.set_retcode(255);
        return;
    }

    let pn = rq.args().get("name").unwrap_or(&ArgValue::default()).as_string().unwrap_or_default();
    rsp.set_message(&format!("File {} created", pn));

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
        let pull_src = rq.args().get("pull").unwrap_or(&ArgValue::default()).as_string().unwrap_or_default();
        match &pull_src {
            s if s.starts_with("file://") => {
                if let Err(err) = copy(PathBuf::from(pull_src.strip_prefix("file://").unwrap_or_default()), PathBuf::from(&pn)) {
                    if strict {
                        rsp.set_retcode(1);
                    }
                    rsp.set_message(&format!("Error copying the file \"{}\": {}", pull_src, err));
                    return;
                }
            }
            _ => {
                if let Err(err) = download(PathBuf::from(&pn), fsr_addr, pull_src) {
                    if strict {
                        rsp.set_retcode(1);
                    }
                    rsp.set_message(&format!("Error pulling the file: {}", err));
                    return;
                }
            }
        };
    } else if let Err(err) = touch(PathBuf::from(&pn)) {
        if strict {
            rsp.set_retcode(1);
        }
        rsp.set_message(&format!("Touch error: {}", err));

        return;
    }

    _ = rsp.cm_set_changed(true);
}
