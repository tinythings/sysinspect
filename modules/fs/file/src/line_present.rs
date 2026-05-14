use libmodcore::{
    response::ModResponse,
    runtime::{ArgValue, ModRequest},
};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

pub fn ensure_line_present(rq: &ModRequest, rsp: &mut ModResponse, strict: bool) {
    rsp.set_retcode(0);
    _ = rsp.cm_set_changed(false);

    let path = PathBuf::from(rq.args().get("name").unwrap_or(&ArgValue::default()).as_string().unwrap_or_default());

    let pattern = rq.args().get("pattern").unwrap_or(&ArgValue::default()).as_string().unwrap_or_default();

    if pattern.is_empty() {
        rsp.set_retcode(1);
        rsp.set_message("Argument \"pattern\" is required and must not be empty");
        return;
    }

    let file_exists = path.exists() && path.is_file();

    if file_exists {
        match read_and_check(&path, &pattern) {
            Ok(true) => {
                rsp.set_message(&format!("Line already present in {}", path.display()));
                return;
            }
            Ok(false) => {}
            Err(err) => {
                rsp.set_retcode(1);
                rsp.set_message(&format!("Error reading {}: {err}", path.display()));
                return;
            }
        }
    }

    if !file_exists && strict {
        rsp.set_retcode(1);
        rsp.set_message(&format!("File {} does not exist", path.display()));
        return;
    }

    match append_line(&path, &pattern, file_exists) {
        Ok(()) => {
            _ = rsp.cm_set_changed(true);
            let action = if file_exists { "added to" } else { "created with line in" };
            rsp.set_message(&format!("Line {} {}", action, path.display()));
        }
        Err(err) => {
            rsp.set_retcode(1);
            rsp.set_message(&format!("Error writing {}: {err}", path.display()));
        }
    }
}

pub(crate) fn read_and_check(path: &PathBuf, pattern: &str) -> Result<bool, std::io::Error> {
    let contents = fs::read_to_string(path)?;
    Ok(contents.lines().any(|line| line == pattern))
}

pub(crate) fn append_line(path: &PathBuf, pattern: &str, exists: bool) -> Result<(), std::io::Error> {
    let needs_newline = exists && {
        let contents = fs::read_to_string(path)?;
        !contents.is_empty() && !contents.ends_with('\n')
    };

    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    if needs_newline {
        f.write_all(b"\n")?;
    }
    f.write_all(pattern.as_bytes())?;
    f.write_all(b"\n")?;

    Ok(())
}
