/*
Filedata manager
 */

use libcommon::SysinspectError;
use libsysinspect::{
    mdescr::mspec::MODEL_FILE_EXT,
    util::iofs::{get_file_sha256, scan_files_sha256},
};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    path::{Component, Path, PathBuf},
};

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
    stale: Vec<String>,

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

    fn safe_rel_path(&self, pth: &str) -> Option<String> {
        let raw = if self.sensors_root.is_empty() {
            pth.to_string()
        } else {
            let rp = pth.trim_start_matches('/');
            let rr = self.sensors_root.trim_start_matches('/').trim_end_matches('/');
            if rp == rr {
                return None;
            }
            let Some(rest) = rp.strip_prefix(rr) else {
                return None;
            };
            if !rest.starts_with('/') {
                return None;
            }
            rest.trim_start_matches('/').to_string()
        };

        let mut out = PathBuf::new();
        for c in Path::new(&raw).components() {
            match c {
                Component::Normal(seg) => out.push(seg),
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
            }
        }
        if out.as_os_str().is_empty() {
            return None;
        }
        Some(out.to_string_lossy().to_string())
    }

    fn init(&mut self, spth: PathBuf) -> Self {
        self.spth = spth;
        let local = scan_files_sha256(self.spth.to_owned(), None);
        let mut expected_rel: HashSet<String> = HashSet::new();
        let mut need_dl: HashMap<String, String> = HashMap::new();
        let mut invalid = 0usize;
        let total = self.files.len();

        for (remote, cs) in self.files.iter() {
            let Some(rel) = self.safe_rel_path(remote) else {
                invalid += 1;
                log::warn!("Skipping unsafe sensor path from master payload: '{}'", remote);
                continue;
            };
            expected_rel.insert(rel.clone());

            let p = self.spth.join(&rel);
            if p.exists() {
                get_file_sha256(p.clone())
                    .map(|c| {
                        if c != *cs {
                            log::warn!("Checksum mismatch for sensor file '{}': expected {}, got {}", rel, cs, c);
                        }
                    })
                    .unwrap_or_else(|e| log::error!("Failed to calculate checksum for '{}': {e}", &p.display()));
            }

            if local.get(&rel) != Some(cs) {
                need_dl.insert(remote.clone(), cs.clone());
            }
        }

        if total > 0 && expected_rel.is_empty() && invalid == total {
            log::warn!(
                "All sensor paths from master payload are unsafe/invalid ({} entries). Skipping stale-file pruning for this sync cycle.",
                total
            );
            self.stale = Vec::new();
        } else {
            self.stale = local.keys().filter(|p| !expected_rel.contains(*p)).cloned().collect::<Vec<String>>();
        }
        self.files = need_dl;
        self.clone()
    }

    pub fn files(&self) -> &HashMap<String, String> {
        &self.files
    }

    pub fn sensors_root(&self) -> &str {
        &self.sensors_root
    }

    pub fn stale_paths(&self) -> &[String] {
        &self.stale
    }

    pub fn local_rel_path(&self, remote: &str) -> Option<String> {
        self.safe_rel_path(remote)
    }
}
