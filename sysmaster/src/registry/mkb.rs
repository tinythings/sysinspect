use super::{CFG_DEFAULT_ROOT, CFG_MASTER_KEY_PRI, CFG_MASTER_KEY_PUB, CFG_MINION_KEYS};
use libsysinspect::{rsa, SysinspectError};
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

    /// Generate keys, if none
    fn gen_keys(&self) -> Result<(), SysinspectError> {
        let prk_pth = self.root.parent().unwrap().join(CFG_MASTER_KEY_PRI);
        let pbk_pth = self.root.parent().unwrap().join(CFG_MASTER_KEY_PUB);

        if prk_pth.exists() || pbk_pth.exists() {
            return Ok(());
        }

        log::debug!("Generating RSA keys...");

        let (prk, pbk) = rsa::keys::keygen(rsa::keys::DEFAULT_KEY_SIZE)?;
        let (prk_pem, pbk_pem) = rsa::keys::to_pem(Some(&prk), Some(&pbk))?;

        if prk_pem.is_none() || pbk_pem.is_none() {
            return Err(SysinspectError::MasterGeneralError(format!("Unable to generate RSA keys")));
        }

        fs::write(prk_pth, prk_pem.unwrap().as_bytes())?;
        fs::write(pbk_pth, pbk_pem.unwrap().as_bytes())?;

        log::debug!("RSA keys saved to the disk");

        Ok(())
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

        self.gen_keys()
    }

    /// Returns a method if a minion Id is known to the key registry.
    pub fn is_registered(&self, mid: &str) -> bool {
        self.keys.contains_key(mid)
    }
}
