use libmodcore::{
    response::ModResponse,
    runtime::{ArgValue, ModRequest},
};
use std::{fs, path::PathBuf};

pub fn ensure_line_absent(rq: &ModRequest, rsp: &mut ModResponse, _strict: bool) {
    rsp.set_retcode(0);
    _ = rsp.cm_set_changed(false);

    let path = PathBuf::from(rq.args().get("name").unwrap_or(&ArgValue::default()).as_string().unwrap_or_default());
    let pattern = rq.args().get("pattern").unwrap_or(&ArgValue::default()).as_string().unwrap_or_default();

    if pattern.is_empty() {
        rsp.set_retcode(1);
        rsp.set_message("Argument \"pattern\" is required and must not be empty");
        return;
    }

    if !path.exists() {
        rsp.set_message(&format!("File {} does not exist (line already absent)", path.display()));
        return;
    }

    let (removed, output) = match remove_matching_lines(&path, &pattern) {
        Ok(r) => r,
        Err(err) => {
            rsp.set_retcode(1);
            rsp.set_message(&format!("Error processing {}: {err}", path.display()));
            return;
        }
    };

    if removed == 0 {
        rsp.set_message(&format!("Line not present in {}", path.display()));
        return;
    }

    if let Err(err) = fs::write(&path, output) {
        rsp.set_retcode(1);
        rsp.set_message(&format!("Error writing {}: {err}", path.display()));
        return;
    }

    _ = rsp.cm_set_changed(true);
    rsp.set_message(&format!("{removed} line(s) removed from {}", path.display()));
}

pub(crate) fn remove_matching_lines(path: &PathBuf, pattern: &str) -> Result<(usize, String), std::io::Error> {
    let contents = fs::read_to_string(path)?;
    let mut matched = 0usize;
    let filtered: Vec<&str> = contents
        .lines()
        .filter(|line| {
            if line == &pattern {
                matched += 1;
                false
            } else {
                true
            }
        })
        .collect();
    Ok((matched, filtered.join("\n") + "\n"))
}
