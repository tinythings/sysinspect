use super::{CFG_DEFAULT_ROOT, CFG_MINION_KEYS};
use libsysinspect::SysinspectError;
use std::{collections::HashMap, fs, path::PathBuf};

/// Registered minion base.
/// Essentially this is just a directory,
/// where collected all public keys from all minions.

#[derive(Debug, Default, Clone)]
pub struct MinionKeyRegistry {
    root: PathBuf,
    keys: HashMap<String, PathBuf>,
}

impl MinionKeyRegistry {
    pub fn new() -> Result<MinionKeyRegistry, SysinspectError> {
        let mut reg =
            MinionKeyRegistry { root: PathBuf::from(CFG_DEFAULT_ROOT).join(CFG_MINION_KEYS), ..MinionKeyRegistry::default() };
        reg.setup()?;

        Ok(reg)
    }

    /// Sets up the registry
    fn setup(&mut self) -> Result<(), SysinspectError> {
        if !self.root.exists() {
            fs::create_dir_all(&self.root)?;
        } else {
            for e in fs::read_dir(&self.root)?.flatten() {
                self.keys
                    .insert(e.file_name().to_str().and_then(|e| e.split('.').next()).unwrap_or_default().to_string(), e.path());
            }
        }

        Ok(())
    }

    /// Returns a method if a minion Id is known to the key registry.
    pub fn is_registered(&self, mid: String) -> bool {
        self.keys.contains_key(&mid)
    }
}
