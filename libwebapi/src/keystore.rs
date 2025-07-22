use libsysinspect::{
    SysinspectError,
    cfg::mmconf::{CFG_MASTER_KEY_PRI, CFG_MASTER_KEY_PUB, MasterConfig},
    rsa::keys::{RsaKey::Private, key_from_file},
};
use rsa::Pkcs1v15Encrypt;
use std::{
    fs::read_to_string,
    io::{ErrorKind, Write},
};
use std::{
    fs::{OpenOptions, create_dir_all},
    sync::OnceLock,
};
use std::{io::Error, path::PathBuf};

/// SysInspect API Keystore keeps client PKCS8 keys.
pub struct SysInspectAPIKeystore {
    keystore: PathBuf,
    sysinspect_root: PathBuf,
}

impl SysInspectAPIKeystore {
    /// Create a new SysInspect API Keystore.
    ///
    /// # Arguments
    /// * `keystore` - The path to the keystore directory.
    ///
    /// # Returns
    /// * A new instance of `SysInspectAPIKeystore`.
    ///
    pub fn new(keystore: PathBuf, sysinspect_root: PathBuf) -> Result<Self, SysinspectError> {
        if keystore.exists() {
            if !keystore.is_dir() {
                return Err(SysinspectError::ObjectNotFound(format!("Keystore path '{}' exists but is not a directory", keystore.display())));
            }
        } else {
            create_dir_all(&keystore).expect("Failed to create keystore directory");
        }

        Ok(Self { keystore, sysinspect_root })
    }

    /// Save a public key to the keystore.
    ///
    /// # Arguments
    /// * `key` - The public key to save.
    /// * `uid` - The user ID associated with the key.
    ///
    /// # Returns
    /// * `Ok(())` if the key was saved successfully.
    /// * `Err(std::io::Error)` if there was an error saving the key.
    ///
    pub fn save_key(&self, uid: &str, key: &str) -> Result<(), SysinspectError> {
        let mut f = OpenOptions::new().create(true).write(true).truncate(true).open(self.keystore.join(format!("{}_public_key.pem", uid)))?;
        f.write_all(key.as_bytes())?;

        Ok(())
    }

    /// Decrypt user data using the master private key.
    ///
    /// # Arguments
    /// * `cipher` - The encrypted data to decrypt.
    ///
    /// # Returns
    /// * `Ok(Vec<u8>)` containing the decrypted data.
    /// * `Err(SysinspectError)` if there was an error decrypting the data.
    ///
    /// This function retrieves the master private key from the keystore and uses it to decrypt the provided cipher text.
    /// If the master key is not found or is not a private key, it returns an error.
    ///
    /// # Errors
    /// * Returns `SysinspectError::ObjectNotFound` if the master key is not found or is not a private key.
    /// * Returns `SysinspectError::RSAError` if there is an error during decryption.
    ///
    pub fn decrypt_user_data(&self, cipher: &[u8]) -> Result<Vec<u8>, SysinspectError> {
        if let Some(prk) = key_from_file(
            self.sysinspect_root
                .join(CFG_MASTER_KEY_PRI)
                .to_str()
                .ok_or_else(|| SysinspectError::ObjectNotFound("Master key not found. :-(".to_string()))?,
        )? {
            match prk {
                Private(prk) => {
                    prk.decrypt(Pkcs1v15Encrypt, &cipher.to_vec()).map_err(|e| SysinspectError::RSAError(format!("RSA decrypt error: {:?}", e)))
                }
                _ => return Err(SysinspectError::ObjectNotFound("Master key is not a private key".to_string())),
            }
        } else {
            return Err(SysinspectError::ObjectNotFound("Master key not found. :-(".to_string()));
        }
    }

    pub fn get_master_key(&self) -> Result<String, SysinspectError> {
        let keypath = self.sysinspect_root.join(CFG_MASTER_KEY_PUB);
        if !keypath.exists() {
            return Err(SysinspectError::ObjectNotFound(format!("Master key file not found at {}", keypath.display())));
        }
        let body = read_to_string(&keypath)
            .map_err(|e| SysinspectError::IoErr(Error::new(ErrorKind::NotFound, format!("Failed to read master key file: {}", e))))?;
        Ok(body)
    }
}

static KEYSTORE: OnceLock<SysInspectAPIKeystore> = OnceLock::new();

/// Get the global SysInspect API Keystore instance.
///
/// # Arguments
/// * `path` - Optional path to the keystore directory. If not provided, defaults to the default SysInspect root directory with `CFG_API_KEYS`.
///
/// # Returns
/// * A reference to the global SysInspect API Keystore instance.
///
/// # Errors
/// * Returns an error if the keystore could not be initialized or opened.
///
pub fn get_webapi_keystore(cfg: &MasterConfig) -> Result<&'static SysInspectAPIKeystore, SysinspectError> {
    if let Some(ks) = KEYSTORE.get() {
        return Ok(ks);
    }

    let ks = SysInspectAPIKeystore::new(cfg.api_keys_root(), cfg.root_dir())?;
    Ok(KEYSTORE.get_or_init(|| ks))
}
