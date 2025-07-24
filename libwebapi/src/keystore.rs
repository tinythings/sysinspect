use libsysinspect::{
    SysinspectError,
    cfg::mmconf::{CFG_MASTER_KEY_PRI, CFG_MASTER_KEY_PUB, MasterConfig},
    rsa::keys::{
        RsaKey::{Private, Public},
        decrypt, encrypt, key_from_file,
    },
};
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
        let mut f = OpenOptions::new().create(true).write(true).truncate(true).open(self.keystore.join(format!("{uid}_public_key.pem")))?;
        f.write_all(key.as_bytes())?;

        Ok(())
    }

    /// Decrypt user data using the master private key.
    /// User supposed to send encrypted data to the server, using the master public key.
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
                Private(prk) => decrypt(prk, cipher.to_vec()).map_err(|_| SysinspectError::RSAError("Failed to decrypt data".to_string())),
                _ => Err(SysinspectError::ObjectNotFound("Master key is not a private key".to_string())),
            }
        } else {
            Err(SysinspectError::ObjectNotFound("Master key not found. :-(".to_string()))
        }
    }

    /// Encrypt user data using the user's public key.
    /// # Arguments
    /// * `uid` - The user ID for which the public key is used.
    /// * `data` - The data to encrypt.
    /// # Returns
    /// * `Ok(Vec<u8>)` containing the encrypted data.
    /// * `Err(SysinspectError)` if there was an error encrypting the data.
    ///
    /// This function retrieves the user's public key from the keystore and uses it to encrypt the provided data.
    /// If the public key is not found or is not a public key, it returns an error.
    ///
    /// # Errors
    /// * Returns `SysinspectError::ObjectNotFound` if the public key file is not found or is not a public key.
    /// * Returns `SysinspectError::RSAError` if there is an error during encryption.
    ///
    pub fn encrypt_user_data(&self, uid: &str, data: &str) -> Result<Vec<u8>, SysinspectError> {
        let pkey = self.keystore.join(format!("{uid}_public_key.pem"));
        if !pkey.exists() {
            return Err(SysinspectError::ObjectNotFound(format!("Public key file not found at {}", pkey.display())));
        }
        let pbk =
            key_from_file(pkey.to_str().ok_or_else(|| SysinspectError::ObjectNotFound(format!("Invalid public key path: {}", pkey.display())))?)?
                .ok_or(SysinspectError::ObjectNotFound(format!("Public key file not found at {}", pkey.display())))?;
        match pbk {
            Public(pbk) => encrypt(pbk, data.as_bytes().to_vec()).map_err(|_| SysinspectError::RSAError("Failed to encrypt data".to_string())),
            _ => Err(SysinspectError::ObjectNotFound("Expected a public key".to_string())),
        }
    }

    /// Get the master public key from the keystore.
    /// # Returns
    /// * `Ok(String)` containing the master public key if it exists.
    /// * `Err(SysinspectError)` if there was an error reading the key file or if the key file does not exist.
    ///
    /// This function reads the master public key from the file specified by `CFG_MASTER_KEY_PUB`.
    /// If the file does not exist, it returns an error.
    /// If the file exists but cannot be read, it returns an `IoErr` error.
    ///
    /// # Errors
    /// * Returns `SysinspectError::ObjectNotFound` if the master key file is not found.
    /// * Returns `SysinspectError::IoErr` if there is an error reading the master key file.
    ///
    pub fn get_master_key(&self) -> Result<String, SysinspectError> {
        let keypath = self.sysinspect_root.join(CFG_MASTER_KEY_PUB);
        if !keypath.exists() {
            return Err(SysinspectError::ObjectNotFound(format!("Master key file not found at {}", keypath.display())));
        }
        let body = read_to_string(&keypath)
            .map_err(|e| SysinspectError::IoErr(Error::new(ErrorKind::NotFound, format!("Failed to read master key file: {e}"))))?;
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
