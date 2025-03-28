/*
Various unsorted utils with the filesystem, IO, files, byte arrays etc
*/

use crate::SysinspectError;
use hex::encode;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::{BufReader, Read},
};
use std::{fs::File, path::PathBuf};
use walkdir::WalkDir;

/// Calculate an SHA265 checksum of a file on the file system.
pub fn get_file_sha256(pth: PathBuf) -> Result<String, SysinspectError> {
    let mut dg = Sha256::new();
    let mut buf = [0u8; 0x2000];
    let mut reader = BufReader::new(File::open(&pth)?);
    loop {
        let brd = reader.read(&mut buf)?;
        if brd == 0 {
            break;
        }
        dg.update(&buf[..brd]);
    }

    Ok(encode(dg.finalize()))
}

/// Scan a given root for any file.
/// Returns a `HashMap` with format `path` to `checksum`.
pub fn scan_files_sha256(pth: PathBuf, ext: Option<&str>) -> HashMap<String, String> {
    let ext = ext.map(|e| e.trim_start_matches('.')); // Just in case :)
    WalkDir::new(&pth)
        .into_iter()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if entry.file_type().is_file() {
                if ext.is_some() && entry.path().extension().and_then(|e| e.to_str()) == ext || ext.is_none() {
                    Some((
                        entry.path().strip_prefix(&pth).ok()?.to_string_lossy().to_string(),
                        get_file_sha256(entry.path().to_path_buf()).unwrap_or_default(),
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect::<HashMap<String, String>>()
}
