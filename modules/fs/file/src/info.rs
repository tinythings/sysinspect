/*
Informational functions
 */

use chrono::{DateTime, Utc};
use libsysinspect::modlib::{
    response::ModResponse,
    runtime::{ArgValue, ModRequest},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, os::unix::fs::MetadataExt};
use std::{fs::File, path::PathBuf};
use std::{io::BufReader, time::SystemTime};
use users::{get_group_by_gid, get_user_by_uid};

#[derive(Serialize, Deserialize, Debug)]
struct FileMetadata {
    #[serde(rename = "path")]
    name: String,

    #[serde(rename = "type")]
    ftype: String,
    is_file: bool,
    is_dir: bool,
    size: u64,

    #[serde(skip_serializing_if = "Option::is_none")]
    created: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    modified: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    accessed: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,

    uid: u32,
    gid: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    group: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "sha256")]
    checksum: Option<String>,
}

/// Convert system time to ISO format
fn time2iso(time: SystemTime) -> Option<String> {
    let datetime: DateTime<Utc> = time.into();
    Some(datetime.to_rfc3339())
}

fn mode_to_octal(mode: u32) -> String {
    format!("{:04o}", mode & 0o7777)
}

/// Calculate SHA256 checksum
fn get_sha256(p: &PathBuf) -> Option<String> {
    let mut hasher = Sha256::new();
    std::io::copy(&mut BufReader::new(File::open(p).ok()?), &mut hasher).ok()?;
    Some(format!("{:x}", hasher.finalize()))
}

/// Collect file information
pub(crate) fn info(rq: &ModRequest, rsp: &mut ModResponse) {
    rsp.set_retcode(0);

    let p = PathBuf::from(rq.args().get("name").unwrap_or(&ArgValue::default()).as_string().unwrap_or_default());

    let meta = match fs::metadata(&p) {
        Ok(m) => m,
        Err(err) => {
            rsp.set_retcode(1);
            rsp.set_message(&format!("Error obtaining file data: {}", err));
            return;
        }
    };

    match serde_json::to_value(&FileMetadata {
        name: p.to_str().unwrap_or_default().to_string(),
        ftype: if meta.is_file() {
            "file".to_string()
        } else if meta.is_dir() {
            "directory".to_string()
        } else {
            "link".to_string()
        },
        is_file: meta.is_file(),
        is_dir: meta.is_dir(),
        size: meta.len(),
        created: meta.created().ok().and_then(time2iso),
        modified: meta.modified().ok().and_then(time2iso),
        accessed: meta.accessed().ok().and_then(time2iso),
        mode: Some(mode_to_octal(meta.mode())),
        uid: meta.uid(),
        gid: meta.gid(),
        user: get_user_by_uid(meta.uid()).map(|i| i.name().to_string_lossy().into_owned()),
        group: get_group_by_gid(meta.gid()).map(|i| i.name().to_string_lossy().into_owned()),
        checksum: get_sha256(&p),
    }) {
        Ok(j) => {
            if let Err(err) = rsp.set_data(j) {
                rsp.set_retcode(1);
                rsp.set_message(&format!("Error sending file data: {}", err));
                return;
            };
        }
        Err(err) => {
            rsp.set_retcode(1);
            rsp.set_message(&format!("Error getting file data: {}", err));
            return;
        }
    };

    rsp.set_message(&format!("Data has been obtained"));
    _ = rsp.cm_set_changed(true);
}
