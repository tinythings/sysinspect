/*
Filedata manager
 */

use libcommon::SysinspectError;
use libsysinspect::{
    mdescr::mspec::MODEL_FILE_EXT,
    util::iofs::{get_file_sha256, scan_files_sha256},
};
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Default, Clone)]
pub struct MinionFiledata {
    // Checksum to file
    stack: HashMap<String, PathBuf>,

    // Path to models (root)
    mpth: PathBuf,
}

impl MinionFiledata {
    /// Constructor
    pub fn new(mpth: PathBuf) -> Result<Self, SysinspectError> {
        let mut instance = Self { mpth, ..Default::default() };
        instance.init();

        Ok(instance)
    }

    pub fn init(&mut self) {
        self.stack = scan_files_sha256(self.mpth.to_owned(), Some(MODEL_FILE_EXT))
            .iter()
            .map(|(f, cs)| (cs.to_owned(), PathBuf::from(f.to_owned())))
            .collect::<HashMap<String, PathBuf>>();
    }

    /// Verify if a corresponding file matches the checksum
    pub fn check_sha256(&self, pth: String, cs: String, relative: bool) -> bool {
        if !self.stack.contains_key(&cs) {
            // Unknown checksum, (re)download required
            return false;
        }

        if let Some(p) = self.stack.get(&cs) {
            let pth = PathBuf::from(pth);
            if relative {
                return pth.ends_with(p);
            } else {
                return pth.eq(p);
            }
        }

        false
    }
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct SensorsFiledata {
    files: HashMap<String, String>,
    sensors_root: String,

    #[serde(skip)]
    stack: HashMap<String, PathBuf>,

    #[serde(skip)]
    spth: PathBuf,
}

impl SensorsFiledata {
    pub fn from_payload(payload: serde_json::Value, spth: PathBuf) -> Result<Self, SysinspectError> {
        match serde_json::from_value::<SensorsFiledata>(payload) {
            Ok(mut sfd) => Ok(sfd.init(spth)),
            Err(err) => Err(SysinspectError::ProtoError(format!("unable to parse sensors filedata: {err}"))),
        }
    }

    /// Path on the server is prefixed with the sensors root, so we need to unprefix it to get the actual path on the minion.
    pub fn unprefix_path(&self, pth: &str) -> String {
        if self.sensors_root.is_empty() {
            return pth.to_string();
        }

        pth.trim_start_matches('/').strip_prefix(self.sensors_root.trim_start_matches('/')).unwrap_or(pth).trim_start_matches('/').to_string()
    }

    fn init(&mut self, spth: PathBuf) -> Self {
        self.spth = spth;
        self.stack = scan_files_sha256(self.spth.to_owned(), None)
            .iter()
            .map(|(f, cs)| (cs.to_owned(), PathBuf::from(f.to_owned())))
            .collect::<HashMap<String, PathBuf>>();

        for (pth, cs) in self.files.iter() {
            let pth = self.unprefix_path(pth);
            let p = self.spth.join(&pth);
            if p.exists() {
                get_file_sha256(p.clone())
                    .map(|c| {
                        if c != *cs {
                            log::warn!("Checksum mismatch for sensor file '{}': expected {}, got {}", pth, cs, c);
                        }
                    })
                    .unwrap_or_else(|e| log::error!("Failed to calculate checksum for '{}': {e}", &p.display()));
            }
        }
        self.files.retain(|_, cs| !self.stack.contains_key(cs));
        self.clone()
    }

    pub fn files(&self) -> &HashMap<String, String> {
        &self.files
    }

    pub fn sensors_root(&self) -> &str {
        &self.sensors_root
    }
}
