/*
Distributed Model Spec.

This package contains utilities to distribute and setup a published
model with all its derivatives across all the minions, so they can
pick it up and process.
*/

use std::{collections::HashMap, path::PathBuf};
use walkdir::WalkDir;

use crate::util::iofs::get_file_sha256;

use super::mspec::MODEL_FILE_EXT;

/// Gets modes files from the `pth` as root.
/// Returns `Vec<String>` of relative paths to the `pth`.
/// Used in for the fileserver, so the minion knows
/// the paths to download.
pub fn model_files(pth: PathBuf) -> HashMap<String, String> {
    let ext = MODEL_FILE_EXT.strip_prefix(".").unwrap_or_default();
    WalkDir::new(&pth)
        .into_iter()
        .filter_map(|entry| {
            let entry = entry.ok()?;

            if entry.file_type().is_file() && entry.path().extension().and_then(|e| e.to_str()) == Some(ext) {
                let relative_path = entry.path().strip_prefix(&pth).ok()?.to_string_lossy().to_string();
                Some((relative_path, get_file_sha256(entry.path().to_path_buf()).unwrap_or_default()))
            } else {
                None
            }
        })
        .collect::<HashMap<String, String>>()
}
