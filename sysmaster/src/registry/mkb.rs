use super::{CFG_DEFAULT_ROOT, CFG_MASTER_KEY_PRI, CFG_MASTER_KEY_PUB, CFG_MINION_KEYS};
use ::rsa::{RsaPrivateKey, RsaPublicKey};
use libsysinspect::{rsa, SysinspectError};
use std::{collections::HashMap, fs, path::PathBuf};

/// Registered minion base.
/// Essentially this is just a directory,
/// where collected all public keys from all minions.

#[derive(Debug, Default, Clone)]
pub struct MinionsKeyRegistry {
    root: PathBuf,
    keys: HashMap<String, Option<RsaPublicKey>>,

    // Master RSA
    ms_prk: Option<RsaPrivateKey>,
    ms_pbk: Option<RsaPublicKey>,
    ms_pbk_pem: Option<String>,
}

impl MinionsKeyRegistry {
    pub fn new() -> Result<MinionsKeyRegistry, SysinspectError> {
        let mut reg =
            MinionsKeyRegistry { root: PathBuf::from(CFG_DEFAULT_ROOT).join(CFG_MINION_KEYS), ..MinionsKeyRegistry::default() };
        reg.setup()?;

        Ok(reg)
    }

    /// Generate keys, if none
    fn init_keys(&mut self) -> Result<(), SysinspectError> {
        let prk_pth = self.root.parent().unwrap().join(CFG_MASTER_KEY_PRI);
        let pbk_pth = self.root.parent().unwrap().join(CFG_MASTER_KEY_PUB);

        if prk_pth.exists() || pbk_pth.exists() {
            let prk_pem = fs::read_to_string(prk_pth)?;
            self.ms_pbk_pem = Some(fs::read_to_string(pbk_pth)?);
            (self.ms_prk, self.ms_pbk) = rsa::keys::from_pem(Some(&prk_pem), self.ms_pbk_pem.as_deref())?;

            if self.ms_pbk.is_none() || self.ms_pbk.is_none() {
                return Err(SysinspectError::MasterGeneralError("Unable to initialise RSA keys".to_string()));
            }

            return Ok(());
        }

        log::debug!("Generating RSA keys...");

        let (prk, pbk) = rsa::keys::keygen(rsa::keys::DEFAULT_KEY_SIZE)?;
        let (prk_pem, pbk_pem) = rsa::keys::to_pem(Some(&prk), Some(&pbk))?;

        if prk_pem.is_none() || pbk_pem.is_none() {
            return Err(SysinspectError::MasterGeneralError("Unable to generate RSA keys".to_string()));
        }

        fs::write(prk_pth, prk_pem.unwrap().as_bytes())?;
        fs::write(pbk_pth, pbk_pem.clone().unwrap().as_bytes())?;

        self.ms_pbk_pem = pbk_pem;

        log::debug!("RSA keys saved to the disk");

        Ok(())
    }

    /// Sets up the registry
    fn setup(&mut self) -> Result<(), SysinspectError> {
        if !self.root.exists() {
            fs::create_dir_all(&self.root)?;
        } else {
            for e in fs::read_dir(&self.root)?.flatten() {
                self.keys.insert(e.file_name().to_str().and_then(|e| e.split('.').next()).unwrap_or_default().to_string(), None);
            }
        }

        self.init_keys()
    }

    /// Returns a method if a minion Id is known to the key registry.
    pub fn is_registered(&self, mid: &str) -> bool {
        self.keys.contains_key(mid)
    }

    /// Get a fingerprint of a master key
    pub fn get_master_key_pem(&self) -> &Option<String> {
        &self.ms_pbk_pem
    }

    /// Add minion key
    pub fn add_mn_key(&mut self, mid: &str, addr: &str, pbk_pem: &str) -> Result<(), SysinspectError> {
        let k_pth = self.root.join(format!("{}.rsa.pub", mid));
        log::debug!("Adding minion key for {mid} at {addr} as {}", k_pth.as_os_str().to_str().unwrap_or_default());
        fs::write(k_pth, pbk_pem)?;

        let (_, pbk) = rsa::keys::from_pem(None, Some(pbk_pem))?;
        if let Some(pbk) = pbk {
            self.keys.insert(mid.to_string(), Some(pbk));
        }
        Ok(())
    }

    /// Lazy-load minion key. By start all keys are only containing minion Ids.
    /// If a key is requested, it is loaded from the disk on demand.
    fn get_mn_key(&mut self, mid: &str) -> Option<RsaPublicKey> {
        log::debug!("Loading RSA key for {mid}");

        if let Some(pbk) = self.keys.get(mid).and_then(|s| s.clone()) {
            return Some(pbk);
        }

        let k_pth = self.root.join(format!("{}.rsa.pub", mid));
        if !k_pth.exists() {
            log::error!("Minion {mid} requests RSA key, but the key is not found!");
            return None;
        }

        match fs::read_to_string(k_pth) {
            Ok(pbk_pem) => {
                if let Ok((_, Some(pbk))) = rsa::keys::from_pem(None, Some(&pbk_pem)) {
                    self.keys.insert(mid.to_string(), Some(pbk.to_owned()));
                    return Some(pbk);
                }
            }
            Err(err) => log::error!("Unable to read minion RSA key: {err}"),
        }
        None
    }

    pub fn remove_mn_key(&self) {}

    pub fn encrypt_with_mn_key(&self) {}

    pub fn encrypt_with_mst_key(&self) {}
}
