use base64::{Engine, engine::general_purpose::STANDARD};
use libsysinspect::{
    SysinspectError,
    rsa::keys::{
        RsaKey::{Private, Public},
        encrypt, key_from_file, key_to_file, keygen,
    },
};
use serde_json::json;
use std::{fs, path::PathBuf};
use syswebclient::{
    apis::{authentication_api::authenticate_user, configuration::Configuration, rsa_public_keys_api::master_key},
    models::AuthRequest,
};

/// SysClient Configuration
/// This struct holds the configuration for the SysClient, including the root directory.
/// It can be extended in the future to include more configuration options.
pub struct SysClientConfiguration {
    pub root: PathBuf,
    pub private_key: String,
    pub public_key: String,
    pub master_public_key: String,
    pub master_url: String,
}

impl SysClientConfiguration {
    /// Returns the path to the private key file, joined with the root directory.
    ///
    /// # Returns
    /// A `PathBuf` representing the full path to the private key file.
    ///
    pub fn privkey_path(&self) -> PathBuf {
        self.root.join(&self.private_key)
    }

    /// Returns the path to the public key file, joined with the root directory.
    ///
    /// # Returns
    /// A `PathBuf` representing the full path to the public key file.
    ///
    pub fn pubkey_path(&self) -> PathBuf {
        self.root.join(&self.public_key)
    }

    /// Returns the path to the master public key file, joined with the root directory.
    ///
    /// # Returns
    /// A `PathBuf` representing the full path to the master public key file.
    ///
    pub fn master_pubkey_path(&self) -> PathBuf {
        self.root.join(&self.master_public_key)
    }

    /// Return API configuration for the SysClient.
    /// This method constructs a `Configuration` object that is used to interact with the SysInspect API.
    /// # Returns
    /// A `Configuration` object with the base path set to the master URL,
    /// user agent set to "sysinspect-client/0.1.0", and a new `reqwest::Client`.
    ///
    pub fn get_api_config(&self) -> Configuration {
        Configuration {
            base_path: self.master_url.clone(),
            user_agent: Some("sysinspect-client/0.1.0".to_string()),
            client: reqwest::Client::new(),
            basic_auth: None,
            oauth_access_token: None,
            bearer_access_token: None,
            api_key: None,
        }
    }
}

impl Default for SysClientConfiguration {
    fn default() -> Self {
        SysClientConfiguration {
            root: PathBuf::from("."),
            private_key: "private.key".to_string(),
            public_key: "public.key".to_string(),
            master_public_key: "master_public.key".to_string(),
            master_url: "http://localhost:4202".to_string(),
        }
    }
}

/// SysClient is the main client for interacting with the SysInspect system.
/// It provides methods to set up RSA encryption, manage configurations, and interact with the system.
pub struct SysClient {
    cfg: SysClientConfiguration,
    sid: String,
}

impl SysClient {
    pub fn new(cfg: SysClientConfiguration) -> Self {
        SysClient { cfg, sid: String::new() }
    }

    /// Setup the SysClient by generating RSA keypair and download Master RSA public key.
    /// Keys are stored where the configuration specifies.
    ///
    /// # Returns
    /// A `Result` that is `Ok(())` if the setup is successful,
    /// or an `Err(SysinspectError)` if there is an error during the setup.
    ///
    pub(crate) async fn setup(&self) -> Result<(), SysinspectError> {
        if !self.cfg.privkey_path().exists() || !self.cfg.pubkey_path().exists() {
            log::debug!("Generating RSA keys...");

            let (prk, pbk) = keygen(2048)?;
            key_to_file(&Private(prk), "./", self.cfg.privkey_path().to_str().unwrap())?;
            key_to_file(&Public(pbk), "./", self.cfg.pubkey_path().to_str().unwrap())?;
        }

        if !self.cfg.master_pubkey_path().exists() {
            let r = master_key(&self.cfg.get_api_config()).await.map_err(|e| {
                SysinspectError::MasterGeneralError(format!("Failed to retrieve master public key (network): {e}"))
            })?;

            if r.key.is_empty() {
                return Err(SysinspectError::MasterGeneralError("Master public key is empty".to_string()));
            }
            fs::write(self.cfg.master_pubkey_path(), r.key.as_bytes()).map_err(SysinspectError::IoErr)?;
        }

        Ok(())
    }

    /// Encrypt data using the master public key.
    /// This method reads the master public key from the file system and uses it to encrypt the provided data.
    /// # Arguments
    /// * `data` - The data to encrypt, provided as a string.
    /// # Returns
    /// A `Result` that is `Ok(Vec<u8>)` containing the encrypted data,
    /// or an `Err(SysinspectError)` if there is an error during the encryption process.
    ///
    pub(crate) fn encrypt(&self, data: &str, pkey: &str) -> Result<Vec<u8>, SysinspectError> {
        let pbk = match key_from_file(pkey)?
            .ok_or(SysinspectError::RSAError("Failed to load RSA key from file".to_string()))?
        {
            Public(ref k) => k.clone(),
            _ => return Err(SysinspectError::RSAError("Expected a public key".to_string())),
        };

        encrypt(pbk, data.as_bytes().to_vec())
            .map_err(|_| SysinspectError::RSAError("Failed to encrypt data".to_string()))
    }

    /// Read the client's public key from the file system.
    /// # Returns
    /// A `Result` that is `Ok(String)` containing the client's public key if successful,
    /// or an `Err(SysinspectError)` if there is an error reading the file.
    ///
    pub fn client_pubkey_pem(&self) -> Result<String, SysinspectError> {
        fs::read_to_string(self.cfg.pubkey_path()).map_err(SysinspectError::IoErr)
    }

    /// Authenticate a user with the SysInspect system.
    /// This method sets up the client first and then performs authentication.
    /// # Arguments
    /// * `uid` - The user ID to authenticate.
    /// * `pwd` - The password for the user.
    /// # Returns
    /// A `Result` that is `Ok(true)` if authentication is successful,
    /// or `Ok(false)` if authentication fails.
    /// If there is an error during the setup or authentication process, it returns an `Err(SysinspectError)`.
    ///
    pub async fn authenticate(&mut self, uid: &str, pwd: &str) -> Result<String, SysinspectError> {
        // Setup the client first
        self.setup().await?;

        // Authenticate the user
        log::debug!("Authenticating user: {uid}");
        let r = authenticate_user(
            &self.cfg.get_api_config(),
            AuthRequest {
                payload: STANDARD.encode(&self.encrypt(
                    &json!({"username": uid, "password": pwd}).to_string(),
                    self.cfg.master_pubkey_path().to_str().unwrap(),
                )?),
                pubkey: self.client_pubkey_pem()?,
            },
        )
        .await
        .map_err(|e| SysinspectError::MasterGeneralError(format!("Authentication error: {e}")))?;

        self.sid = r.sid.unwrap().unwrap();
        Ok(self.sid.clone())
    }
}
