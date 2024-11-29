pub mod dataconv;
pub mod iofs;

use crate::SysinspectError;
use std::{fs, io, path::PathBuf};
use uuid::Uuid;

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
