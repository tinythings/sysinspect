pub mod dataconv;
pub mod iofs;
pub mod sys;
pub mod tty;

use libcommon::SysinspectError;
use once_cell::sync::Lazy;
use regex::Regex;
use std::{fs, io, path::PathBuf};
use uuid::Uuid;

static ANSI_ESCAPE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\x1b\[[0-9;]*m").expect("ansi regex should compile"));

/// The `/etc/machine-id` is not always present, especially
/// on the custom embedded systems. However, this file is used
/// to identify a minion.
///
/// Write the `/etc/machine-id` (or other location), if not any yet.
pub fn write_machine_id(p: Option<PathBuf>) -> Result<(), SysinspectError> {
    let p = p.unwrap_or(PathBuf::from("/etc/machine-id"));
    if !p.exists() {
        if let Err(err) = fs::write(p, Uuid::new_v4().to_string().replace("-", "")) {
            return Err(SysinspectError::IoErr(err));
        }
    } else {
        return Err(SysinspectError::IoErr(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("File \"{}\" already exists", p.to_str().unwrap_or_default()),
        )));
    }

    Ok(())
}

pub fn pad_visible(text: &str, width: usize) -> String {
    let visible = ANSI_ESCAPE_RE.replace_all(text, "").chars().count();
    if visible >= width { text.to_string() } else { format!("{text}{}", " ".repeat(width - visible)) }
}
