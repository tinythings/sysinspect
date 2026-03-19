/*
RSA keys manager
 */

use libcommon::SysinspectError;
use libsysinspect::{
    cfg::mmconf::{CFG_MASTER_KEY_PUB, CFG_MINION_RSA_PRV, CFG_MINION_RSA_PUB},
    transport::TransportStore,
};
use libsysproto::secure::SECURE_PROTOCOL_VERSION;
use rsa::{RsaPrivateKey, RsaPublicKey};
use std::{fs, path::PathBuf};

#[derive(Debug, Default, Clone)]
pub struct MinionRSAKeyManager {
    root: PathBuf,

    // RSA
    mn_prk: Option<RsaPrivateKey>,
    mn_pbk: Option<RsaPublicKey>,
    mn_pbk_pem: String,
}

impl MinionRSAKeyManager {
    /// Initiate Minion's RSA key manager. Parameter `root` is
    /// optional, if configuration contains alternative Minion root.
    pub fn new(root: PathBuf) -> Result<MinionRSAKeyManager, SysinspectError> {
        let mut keyman = MinionRSAKeyManager { root, ..Default::default() };

        keyman.setup()?;
        Ok(keyman)
    }

    /// Initialise RSA keys, if none
    fn init_keys(&mut self) -> Result<(), SysinspectError> {
        let prk_pth = self.root.join(CFG_MINION_RSA_PRV);
        let pbk_pth = self.root.join(CFG_MINION_RSA_PUB);

        // Exists already?
        if prk_pth.exists() || pbk_pth.exists() {
            let prk_pem = fs::read_to_string(prk_pth)?;
            let pbk_pem = fs::read_to_string(pbk_pth)?;
            (self.mn_prk, self.mn_pbk) = libsysinspect::rsa::keys::from_pem(Some(&prk_pem), Some(&pbk_pem))?;
            self.mn_pbk_pem = pbk_pem;

            return Ok(());
        }

        // Create RSA keypair
        log::info!("Creating RSA keypair...");

        let (prk, pbk) = libsysinspect::rsa::keys::keygen(libsysinspect::rsa::keys::DEFAULT_KEY_SIZE)?;
        let (prk_pem, pbk_pem) = libsysinspect::rsa::keys::to_pem(Some(&prk), Some(&pbk))?;

        if prk_pem.is_none() || pbk_pem.is_none() {
            return Err(SysinspectError::MinionGeneralError("Error generating new RSA keys".to_string()));
        }

        self.mn_pbk_pem = pbk_pem.to_owned().unwrap();
        self.mn_prk = Some(prk);
        self.mn_pbk = Some(pbk);

        log::info!("Writing public keys to {:?}", pbk_pth.parent());

        fs::write(prk_pth, prk_pem.unwrap())?;
        fs::write(pbk_pth, pbk_pem.unwrap())?;

        log::info!("RSA keypair created");

        Ok(())
    }

    /// Setup the RSA key manager
    fn setup(&mut self) -> Result<(), SysinspectError> {
        self.init_keys()?;
        Ok(())
    }

    /// Get RSA PEM pubkey
    pub fn get_pubkey_pem(&self) -> String {
        self.mn_pbk_pem.to_owned()
    }

    pub fn get_pubkey_fingerprint(&self) -> Result<String, SysinspectError> {
        libsysinspect::rsa::keys::get_fingerprint(
            self.mn_pbk.as_ref().ok_or_else(|| SysinspectError::RSAError("Minion public key is not loaded".to_string()))?,
        )
        .map_err(|err| SysinspectError::RSAError(err.to_string()))
    }

    /// Return the trusted master RSA public key when it is already present on disk.
    pub fn master_public_key(&self) -> Result<Option<RsaPublicKey>, SysinspectError> {
        let master_pem_path = self.root.join(CFG_MASTER_KEY_PUB);
        if !master_pem_path.exists() {
            return Ok(None);
        }
        Ok(libsysinspect::rsa::keys::from_pem(None, Some(&fs::read_to_string(master_pem_path)?))?.1)
    }

    /// Return the loaded minion RSA private key used for secure bootstrap creation.
    pub fn private_key(&self) -> Result<RsaPrivateKey, SysinspectError> {
        self.mn_prk.clone().ok_or_else(|| SysinspectError::RSAError("Minion private key is not loaded".to_string()))
    }

    pub fn ensure_transport_state(&self, minion_id: &str) -> Result<bool, SysinspectError> {
        let master_pem_path = self.root.join(CFG_MASTER_KEY_PUB);
        if !master_pem_path.exists() {
            return Ok(false);
        }

        let (_, master_pbk) = libsysinspect::rsa::keys::from_pem(None, Some(&fs::read_to_string(master_pem_path)?))?;
        let master_fingerprint = libsysinspect::rsa::keys::get_fingerprint(
            &master_pbk.ok_or_else(|| SysinspectError::RSAError("Master public key is not loaded".to_string()))?,
        )
        .map_err(|err| SysinspectError::RSAError(err.to_string()))?;
        let store = TransportStore::new(self.root.join("transport/master/state.json"))?;
        let _ = store.ensure_automatic_peer(minion_id, &master_fingerprint, &self.get_pubkey_fingerprint()?, SECURE_PROTOCOL_VERSION)?;
        Ok(true)
    }
}
