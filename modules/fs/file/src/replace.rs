use libmodcore::{
    response::ModResponse,
    runtime::{ArgValue, ModRequest},
};
use std::{fs, path::PathBuf};

pub fn replace_content(rq: &ModRequest, rsp: &mut ModResponse, strict: bool) {
    rsp.set_retcode(0);
    _ = rsp.cm_set_changed(false);

    let path = PathBuf::from(rq.args().get("name").unwrap_or(&ArgValue::default()).as_string().unwrap_or_default());
    let pattern = rq.args().get("pattern").unwrap_or(&ArgValue::default()).as_string().unwrap_or_default();
    let value = rq.args().get("value").unwrap_or(&ArgValue::default()).as_string().unwrap_or_default();

    if pattern.is_empty() {
        rsp.set_retcode(1);
        rsp.set_message("Argument \"pattern\" is required and must not be empty");
        return;
    }

    if !path.exists() {
        if strict {
            rsp.set_retcode(1);
        }
        rsp.set_message(&format!("File {} does not exist", path.display()));
        return;
    }

    let (count, output) = match do_replace(&path, &pattern, &value) {
        Ok(r) => r,
        Err(err) => {
            rsp.set_retcode(1);
            rsp.set_message(&format!("Error processing {}: {err}", path.display()));
            return;
        }
    };

    if count == 0 {
        rsp.set_message(&format!("No matches found in {}", path.display()));
        return;
    }

    if let Err(err) = fs::write(&path, output) {
        rsp.set_retcode(1);
        rsp.set_message(&format!("Error writing {}: {err}", path.display()));
        return;
    }

    _ = rsp.cm_set_changed(true);
    rsp.set_message(&format!("{count} replacement(s) in {}", path.display()));
}

pub(crate) fn do_replace(path: &PathBuf, pattern: &str, value: &str) -> Result<(usize, String), std::io::Error> {
    let contents = fs::read_to_string(path)?;
    let mut count = 0usize;
    let output = contents
        .lines()
        .map(|line| {
            if line.contains(pattern) {
                count += 1;
                line.replace(pattern, value)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    // Preserve trailing newline if original had one
    let output = if contents.ends_with('\n') { output + "\n" } else { output };
    Ok((count, output))
}
