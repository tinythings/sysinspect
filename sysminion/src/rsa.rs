/*
RSA keys manager
 */

use libcommon::SysinspectError;
use libsysinspect::cfg::mmconf::{CFG_MINION_RSA_PRV, CFG_MINION_RSA_PUB};
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
}
