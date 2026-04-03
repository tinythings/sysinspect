use ::rsa::{RsaPrivateKey, RsaPublicKey};
use libcommon::SysinspectError;
use libsysinspect::{
    cfg::mmconf::{CFG_MASTER_KEY_PRI, CFG_MASTER_KEY_PUB, CFG_TRANSPORT_ROOT, CFG_TRANSPORT_STATE},
    rsa,
    transport::{TransportStore, transport_minion_root},
};
use libsysproto::secure::SECURE_PROTOCOL_VERSION;
use std::{collections::HashMap, fs, io, path::PathBuf};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RegistrationStatus {
    Added,
    Exists,
    Conflict { current: String, requested: String },
}

impl MinionsKeyRegistry {
    pub fn new(root: PathBuf) -> Result<MinionsKeyRegistry, SysinspectError> {
        let mut reg = MinionsKeyRegistry { root, ..MinionsKeyRegistry::default() };
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

            if self.ms_prk.is_none() || self.ms_pbk.is_none() {
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

        self.ms_prk = Some(prk);
        self.ms_pbk = Some(pbk);
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
                if let Some(mid) = e.file_name().to_str().and_then(|e| e.split('.').next())
                    && !mid.is_empty()
                {
                    self.keys.insert(mid.to_string(), None);
                }
            }
        }

        self.init_keys()?;
        self.backfill_transport_state()
    }

    /// Returns a method if a minion Id is known to the key registry.
    pub fn is_registered(&self, mid: &str) -> bool {
        self.keys.contains_key(mid)
    }

    /// Get a fingerprint of a master key
    pub fn get_master_key_pem(&self) -> &Option<String> {
        &self.ms_pbk_pem
    }

    /// Return the loaded master RSA private key used for secure bootstrap acceptance.
    pub fn master_private_key(&self) -> Result<RsaPrivateKey, SysinspectError> {
        self.ms_prk.clone().ok_or_else(|| SysinspectError::MasterGeneralError("Master RSA private key is not loaded".to_string()))
    }

    pub fn get_master_key_fingerprint(&self) -> Result<String, SysinspectError> {
        rsa::keys::get_fingerprint(
            self.ms_pbk.as_ref().ok_or_else(|| SysinspectError::MasterGeneralError("Master RSA public key is not loaded".to_string()))?,
        )
        .map_err(|err| SysinspectError::RSAError(err.to_string()))
    }

    /// Add or verify one minion RSA key in the registry.
    pub fn add_mn_key(&mut self, mid: &str, addr: &str, pbk_pem: &str) -> Result<RegistrationStatus, SysinspectError> {
        let (_, pbk) = rsa::keys::from_pem(None, Some(pbk_pem))?;
        let pbk = pbk.ok_or_else(|| SysinspectError::MasterGeneralError(format!("Registration key for {mid} is missing a public RSA key")))?;
        let requested = rsa::keys::get_fingerprint(&pbk).map_err(|err| SysinspectError::RSAError(err.to_string()))?;
        if let Some(current) = self.get_mn_key(mid) {
            let current = rsa::keys::get_fingerprint(&current).map_err(|err| SysinspectError::RSAError(err.to_string()))?;
            return Ok(if current == requested { RegistrationStatus::Exists } else { RegistrationStatus::Conflict { current, requested } });
        }

        let k_pth = self.root.join(format!("{mid}.rsa.pub"));
        log::debug!("Adding minion key for {mid} at {addr} as {}", k_pth.as_os_str().to_str().unwrap_or_default());
        fs::write(k_pth, pbk_pem)?;
        self.ensure_transport_state(mid, &pbk)?;
        self.keys.insert(mid.to_string(), Some(pbk));
        Ok(RegistrationStatus::Added)
    }

    pub fn get_mn_key_fingerprint(&mut self, mid: &str) -> Result<String, SysinspectError> {
        rsa::keys::get_fingerprint(
            &self.get_mn_key(mid).ok_or_else(|| SysinspectError::MasterGeneralError(format!("RSA public key for minion {mid} is not loaded")))?,
        )
        .map_err(|err| SysinspectError::RSAError(err.to_string()))
    }

    /// Return the loaded minion RSA public key used for secure bootstrap verification.
    pub fn minion_public_key(&mut self, mid: &str) -> Result<RsaPublicKey, SysinspectError> {
        self.get_mn_key(mid).ok_or_else(|| SysinspectError::MasterGeneralError(format!("RSA public key for minion {mid} is not loaded")))
    }

    /// Lazy-load minion key. By start all keys are only containing minion Ids.
    /// If a key is requested, it is loaded from the disk on demand.
    fn get_mn_key(&mut self, mid: &str) -> Option<RsaPublicKey> {
        log::debug!("Loading RSA key for {mid}");

        if let Some(pbk) = self.keys.get(mid).and_then(|s| s.clone()) {
            return Some(pbk);
        }

        let k_pth = self.root.join(format!("{mid}.rsa.pub"));
        if !k_pth.exists() {
            log::info!("Minion {mid} requested initial RSA key bootstrap; no stored key exists yet");
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

    /// Remove minion public key from the store
    pub fn remove_mn_key(&mut self, mid: &str) -> Result<(), SysinspectError> {
        let k_pth = self.root.join(format!("{mid}.rsa.pub"));
        if k_pth.exists() {
            fs::remove_file(k_pth)?;
            self.keys.remove(mid);
        } else {
            return Err(SysinspectError::IoErr(io::Error::new(io::ErrorKind::NotFound, format!("No RSA public key found for {mid}"))));
        }

        // Keep registration cleanup symmetric by removing managed transport metadata too.
        let transport_root = transport_minion_root(&self.transport_root()?, mid)?;
        if transport_root.exists() {
            fs::remove_dir_all(transport_root)?;
        }

        Ok(())
    }

    fn transport_root(&self) -> Result<PathBuf, SysinspectError> {
        Ok(self
            .root
            .parent()
            .ok_or_else(|| SysinspectError::ConfigError(format!("Registry root {} has no parent for transport metadata", self.root.display())))?
            .join(CFG_TRANSPORT_ROOT))
    }

    fn backfill_transport_state(&mut self) -> Result<(), SysinspectError> {
        for mid in self.keys.keys().cloned().collect::<Vec<_>>() {
            if let Some(pbk) = self.get_mn_key(&mid) {
                self.ensure_transport_state(&mid, &pbk)?;
            }
        }
        Ok(())
    }

    fn ensure_transport_state(&self, mid: &str, pbk: &RsaPublicKey) -> Result<(), SysinspectError> {
        let store = TransportStore::new(transport_minion_root(&self.transport_root()?, mid)?.join(CFG_TRANSPORT_STATE))?;
        let _ = store.ensure_automatic_peer(
            mid,
            &self.get_master_key_fingerprint()?,
            &rsa::keys::get_fingerprint(pbk).map_err(|err| SysinspectError::RSAError(err.to_string()))?,
            SECURE_PROTOCOL_VERSION,
        )?;
        Ok(())
    }
}
