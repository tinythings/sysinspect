/*
Filedata manager
 */

use libsysinspect::{mdescr::mspec::MODEL_FILE_EXT, util::iofs::scan_files_sha256, SysinspectError};
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
