use base64::{Engine, engine::general_purpose::STANDARD};
use libsysinspect::{
    SysinspectError,
    rsa::keys::{
        RsaKey::{Private, Public},
        decrypt, encrypt, key_from_file, key_to_file, keygen,
    },
};
use serde_json::{Value, json};
use sodiumoxide::crypto::secretbox::{self, Key, Nonce, gen_nonce};
use std::{fs, path::PathBuf};
use syswebclient::{
    apis::{
        configuration::Configuration, minions_api::query_handler, rsa_keys_api::master_key,
        system_api::authenticate_user,
    },
    models::{AuthRequest, QueryResponse},
};

/// SysClient Configuration
/// This struct holds the configuration for the SysClient, including the root directory.
/// It can be extended in the future to include more configuration options.
///
/// # Fields
/// * `root` - The root directory where keys and other files are stored.
/// * `private_key` - The filename of the private key.
/// * `public_key` - The filename of the public key.
/// * `master_public_key` - The filename of the master public key.
/// * `master_url` - The URL of the SysInspect master server.
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
    pub fn privkey_path(&self) -> PathBuf {
        self.root.join(&self.private_key)
    }

    /// Returns the path to the public key file, joined with the root directory.
    ///
    /// # Returns
    /// A `PathBuf` representing the full path to the public key file.
    pub fn pubkey_path(&self) -> PathBuf {
        self.root.join(&self.public_key)
    }

    /// Returns the path to the master public key file, joined with the root directory.
    ///
    /// # Returns
    /// A `PathBuf` representing the full path to the master public key file.
    pub fn master_pubkey_path(&self) -> PathBuf {
        self.root.join(&self.master_public_key)
    }

    /// Return API configuration for the SysClient.
    /// This method constructs a `Configuration` object that is used to interact with the SysInspect API.
    /// # Returns
    /// A `Configuration` object with the base path set to the master URL,
    /// user agent set to "sysinspect-client/0.1.0", and a new `reqwest::Client`.
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
/// It handles user authentication, key management, and data encryption/decryption.
///
/// # Fields
/// * `cfg` - The configuration for the SysClient, which includes paths to keys and the master URL.
/// * `sid` - The session ID for the authenticated user.
/// * `symkey` - The symmetric key used for encrypting and decrypting data after authentication.
pub struct SysClient {
    cfg: SysClientConfiguration,
    sid: String,
    symkey: Vec<u8>,
}

impl SysClient {
    pub fn new(cfg: SysClientConfiguration) -> Self {
        SysClient { cfg, sid: String::new(), symkey: Vec::new() }
    }

    /// Setup the SysClient by generating RSA keypair and download Master RSA public key.
    /// Keys are stored where the configuration specifies.
    ///
    /// # Returns
    /// A `Result` that is `Ok(())` if the setup is successful,
    /// or an `Err(SysinspectError)` if there is an error during the setup.
    pub(crate) async fn setup(&self) -> Result<(), SysinspectError> {
        if !self.cfg.privkey_path().exists() || !self.cfg.pubkey_path().exists() {
            log::debug!("Generating RSA keys...");

            let (prk, pbk) = keygen(2048)?;
            key_to_file(&Private(prk), "./", self.cfg.privkey_path().to_str().unwrap())?;
            key_to_file(&Public(pbk), "./", self.cfg.pubkey_path().to_str().unwrap())?;
            log::debug!("RSA keys generated successfully.");
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

    /// Encode data to Base64 format.
    /// This method uses the `base64` crate to encode the provided data into a Base64 string.
    /// # Arguments
    /// * `data` - The data to encode, provided as a byte slice.
    /// # Returns
    /// A `String` containing the Base64-encoded representation of the input data.
    /// # Errors
    /// * This function does not return an error; it will always return a valid Base64 string.
    pub(crate) fn b64encode(&self, data: &[u8]) -> String {
        STANDARD.encode(data)
    }

    /// Decode Base64-encoded data.
    /// This method uses the `base64` crate to decode the provided Base64 string into a byte vector.
    /// # Arguments
    /// * `data` - The Base64-encoded data to decode, provided as a string.
    /// # Returns
    /// A `Result` that is `Ok(Vec<u8>)` containing the decoded data,
    /// or an `Err(SysinspectError)` if there is an error during the decoding process.
    /// # Errors
    /// * Returns `SysinspectError::SerializationError` if the provided data is not valid Base64.
    ///
    /// This function will return an error if the input string cannot be decoded into valid Base64 data.
    pub(crate) fn b64decode(&self, data: &str) -> Result<Vec<u8>, SysinspectError> {
        STANDARD
            .decode(data)
            .map_err(|e| SysinspectError::SerializationError(format!("Failed to decode base64 data: {e}")))
    }

    /// Encrypt data using a public key (master or own).
    /// This method reads the public key from the file system and uses it to encrypt the provided data.
    /// # Arguments
    /// * `data` - The data to encrypt, provided as a string.
    /// # Returns
    /// A `Result` that is `Ok(Vec<u8>)` containing the encrypted data,
    /// or an `Err(SysinspectError)` if there is an error during the encryption process.
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

    /// Decrypt data using the private key.
    /// This method reads the private key from the file system and uses it to decrypt the provided data.
    /// # Arguments
    /// * `data` - The encrypted data to decrypt, provided as a byte slice.
    /// # Returns
    /// A `Result` that is `Ok(String)` containing the decrypted data as a string,
    /// or an `Err(SysinspectError)` if there is an error during the decryption process.
    ///
    /// # Errors
    /// * Returns `SysinspectError::RSAError` if there is an error during decryption,
    /// * Returns `SysinspectError::SerializationError` if the decrypted data cannot be converted to a string.
    ///
    /// This function expects the private key to be in PEM format and stored in the file specified
    /// by `self.cfg.privkey_path()`. If the file does not exist or is not a valid private key,
    /// it will return an error.
    pub(crate) fn decrypt(&self, data: &[u8]) -> Result<String, SysinspectError> {
        let prk = match key_from_file(self.cfg.privkey_path().to_str().unwrap())?
            .ok_or(SysinspectError::RSAError("Failed to load RSA key from file".to_string()))?
        {
            Private(ref k) => k.clone(),
            _ => return Err(SysinspectError::RSAError("Expected a private key".to_string())),
        };

        let data =
            decrypt(prk, data.to_vec()).map_err(|_| SysinspectError::RSAError("Failed to decrypt data".to_string()))?;

        String::from_utf8(data)
            .map_err(|_| SysinspectError::SerializationError("Failed to decode decrypted data".to_string()))
    }

    /// Read the client's public key from the file system.
    ///
    /// # Returns
    /// A `Result` that is `Ok(String)` containing the client's public key if successful,
    /// or an `Err(SysinspectError)` if there is an error reading the file.
    pub async fn client_pubkey_pem(&self) -> Result<String, SysinspectError> {
        fs::read_to_string(self.cfg.pubkey_path()).map_err(SysinspectError::IoErr)
    }

    /// Convert a JSON payload to a symmetric encrypted payload.
    /// This method generates a nonce, serializes the payload to JSON, and encrypts it
    /// using the symmetric key stored in `self.symkey`.
    ///
    /// # Arguments
    /// * `payload` - The JSON payload to encrypt, provided as a `serde_json::Value`.
    /// # Returns
    /// A `Result` that is `Ok((Vec<u8>, Vec<u8>))` containing the nonce and the encrypted data,
    /// or an `Err(SysinspectError)` if there is an error during serialization or encryption.
    ///
    /// # Errors
    /// * Returns `SysinspectError::SerializationError` if the payload cannot be serialized to JSON,
    /// * Returns `SysinspectError::SerializationError` if the symmetric key is not valid (i.e., not 32 bytes long),
    /// * Returns `SysinspectError::SerializationError` if the encryption fails for any reason.
    ///
    /// This function uses the `sodiumoxide` library for encryption,
    /// specifically the `secretbox` module for symmetric encryption.
    /// It expects the symmetric key to be 32 bytes long,
    /// which is the required length for the `secretbox::Key`.
    ///
    /// The nonce is generated using `secretbox::gen_nonce()`, which creates a new nonce for each encryption operation.
    /// The payload is serialized to a byte vector using `serde_json::to_vec()`.
    /// If the serialization fails, it returns a `SysinspectError::SerializationError`.
    /// The symmetric key is created from the `self.symkey` field, which is expected to be a 32-byte slice.
    /// If the key is not valid, it returns a `SysinspectError::SerializationError`.
    /// The encrypted data is produced using `secretbox::seal()`, which takes the serialized data, nonce, and symmetric key.
    /// If the encryption fails, it returns a `SysinspectError::SerializationError`.
    /// The function returns a tuple containing the nonce and the encrypted data as byte vectors.
    /// The nonce is returned as a `Vec<u8>` for easy transmission, and the encrypted data is also returned as a `Vec<u8>`.
    /// This allows the caller to use the nonce and encrypted data for further processing,
    /// such as sending it over a network or storing it securely.
    pub async fn to_payload(&self, payload: &Value) -> Result<(Vec<u8>, Vec<u8>), SysinspectError> {
        let nonce = gen_nonce();
        Ok((
            nonce.0.to_vec(),
            secretbox::seal(
                &serde_json::to_vec(payload).map_err(|e| SysinspectError::SerializationError(e.to_string()))?,
                &nonce,
                &Key::from_slice(&self.symkey)
                    .ok_or_else(|| SysinspectError::SerializationError("Invalid symmetric key length".to_string()))?,
            ),
        ))
    }

    /// Decrypt a payload using the symmetric key and nonce.
    /// This method takes a nonce and a payload, decrypts the payload using the symmetric key,
    /// and deserializes the decrypted data into a `serde_json::Value`.
    /// # Arguments
    /// * `nonce` - The nonce used for decryption, provided as a byte slice.
    /// * `payload` - The encrypted payload to decrypt, provided as a byte slice.
    /// # Returns
    /// A `Result` that is `Ok(Value)` containing the deserialized JSON value if successful,
    /// or an `Err(SysinspectError)` if there is an error during decryption or deserialization.
    ///
    /// # Errors
    /// * Returns `SysinspectError::SerializationError` if the nonce is not valid (i.e., not 24 bytes long),
    /// * Returns `SysinspectError::SerializationError` if the symmetric key is not valid (i.e., not 32 bytes long),
    /// * Returns `SysinspectError::SerializationError` if the decryption fails,
    /// * Returns `SysinspectError::DeserializationError` if the decrypted data cannot be deserialized into a `serde_json::Value`.
    ///
    /// This function uses the `sodiumoxide` library for decryption,
    /// specifically the `secretbox` module for symmetric decryption.
    /// It expects the nonce to be 24 bytes long, which is the required length for the `secretbox::Nonce`.
    /// The symmetric key is expected to be 32 bytes long,
    /// which is the required length for the `secretbox::Key`.
    /// The function first checks the length of the nonce and symmetric key,
    /// and if they are not valid, it returns a `SysinspectError::SerializationError`.
    /// It then attempts to decrypt the payload using `secretbox::open()`, which takes the payload, nonce, and symmetric key.
    /// If the decryption fails, it returns a `SysinspectError::SerializationError`.
    /// Finally, it deserializes the decrypted data into a `serde_json::Value` using `serde_json::from_slice()`.
    /// If the deserialization fails, it returns a `SysinspectError::DeserializationError`.
    /// If all operations are successful, it returns the deserialized `Value` as a result.
    /// This allows the caller to retrieve the original JSON data that was encrypted and sent as a payload.
    /// The decrypted data is expected to be in JSON format,
    /// and the function will return a `serde_json::Value` that can be used for further processing or analysis.
    /// This is useful for applications that need to securely transmit JSON data,
    /// such as configuration settings, user data, or other structured information,
    /// while ensuring that the data remains confidential and tamper-proof during transmission.
    pub async fn from_payload(&self, nonce: &[u8], payload: &[u8]) -> Result<Value, SysinspectError> {
        let nonce =
            Nonce::from_slice(nonce).ok_or(SysinspectError::SerializationError("Invalid nonce length".to_string()))?;
        let key = Key::from_slice(&self.symkey)
            .ok_or_else(|| SysinspectError::SerializationError("Invalid symmetric key length".to_string()))?;

        let data = secretbox::open(payload, &nonce, &key)
            .map_err(|_| SysinspectError::SerializationError("Failed to decrypt payload".to_string()))?;

        serde_json::from_slice(&data).map_err(|e| SysinspectError::DeserializationError(e.to_string()))
    }

    /// Authenticate a user with the SysInspect system.
    /// This method sets up the client first and then performs authentication.
    ///
    /// # Arguments
    /// * `uid` - The user ID to authenticate.
    /// * `pwd` - The password for the user.
    ///
    ///  # Returns
    /// A `Result` that is `Ok(true)` if authentication is successful,
    /// or `Ok(false)` if authentication fails.
    /// If there is an error during the setup or authentication process, it returns an `Err(SysinspectError)`.
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
                pubkey: self.client_pubkey_pem().await?,
            },
        )
        .await
        .map_err(|e| SysinspectError::MasterGeneralError(format!("Authentication error: {e}")))?;

        self.symkey =
            hex::decode(
                self.decrypt(&STANDARD.decode(&r.symkey_cipher).map_err(|e| {
                    SysinspectError::SerializationError(format!("Failed to decode base64 symkey: {e}"))
                })?)
                .map_err(|e| SysinspectError::SerializationError(format!("Failed to decrypt symkey: {e}")))?,
            )
            .map_err(|e| SysinspectError::SerializationError(format!("Failed to decode hex symkey: {e}")))?;

        self.sid = self
            .decrypt(
                &STANDARD
                    .decode(&r.sid_cipher)
                    .map_err(|e| SysinspectError::SerializationError(format!("Failed to decode base64 SID: {e}")))?,
            )
            .map_err(|e| SysinspectError::SerializationError(format!("Failed to decrypt SID: {e}")))?;

        log::debug!("Authenticated user: {uid}, session ID: {}, symmetric key: {:x?}", self.sid, self.symkey);

        Ok(self.sid.clone())
    }

    /// Query the SysInspect system with a given query string.
    /// This method requires the client to be authenticated (i.e., `sid` must not be empty).
    ///
    /// # Arguments
    /// * `query` - The query string to send to the SysInspect system.
    ///
    /// # Returns
    /// A `Result` that is `Ok(String)` containing the response from the SysInspect system,
    /// or an `Err(SysinspectError)` if there is an error during the query process.
    ///
    /// # Errors
    /// * Returns `SysinspectError::MasterGeneralError` if the client is not authenticated (i.e., `sid` is empty),
    /// * Returns `SysinspectError::MasterGeneralError` if there is an error during the query process, such as network issues or server errors.
    ///
    /// This function constructs a JSON payload containing the session ID and the query,
    /// encodes it, and sends it to the SysInspect system using the `query_handler` API.
    /// It expects the SysInspect system to respond with a string, which is returned as the result.
    pub async fn query(
        &self, model: &str, query: &str, traits: &str, mid: &str, context: Value,
    ) -> Result<QueryResponse, SysinspectError> {
        if self.sid.is_empty() {
            return Err(SysinspectError::MasterGeneralError("Client is not authenticated".to_string()));
        }

        let payload = json!({
            "model": model,
            "query": query,
            "traits": traits,
            "mid": mid,
            "context": context,
        });

        let (nonce, payload) = self.to_payload(&payload).await?;
        let query_request = syswebclient::models::QueryRequest {
            nonce: STANDARD.encode(nonce),
            payload: STANDARD.encode(payload),
            sid_rsa: self.b64encode(&self.encrypt(&self.sid.clone(), self.cfg.master_pubkey_path().to_str().unwrap())?),
        };

        let response = query_handler(&self.cfg.get_api_config(), query_request)
            .await
            .map_err(|e| SysinspectError::MasterGeneralError(format!("Query error: {e}")))?;

        Ok(response)
    }

    /// Retrieve the list of available models from the SysInspect system.
    /// This method requires the client to be authenticated.
    /// # Returns
    /// A `Result` that is `Ok(ModelNameResponse)` containing the list of models,
    /// or an `Err(SysinspectError)` if there is an error during the retrieval process.
    /// # Errors
    /// * Returns `SysinspectError::MasterGeneralError` if there is an error during the retrieval process, such as network issues or server errors.
    ///
    /// Calls the `list_models` API to fetch available models from the SysInspect system.
    /// Returns a `ModelNameResponse` containing the list of models on success, or a `SysinspectError` if the API call fails.
    /// This enables the caller to access the models provided by the SysInspect system.
    pub async fn models(&self) -> Result<syswebclient::models::ModelNameResponse, SysinspectError> {
        if self.sid.is_empty() {
            return Err(SysinspectError::MasterGeneralError("Client is not authenticated".to_string()));
        }

        let response = syswebclient::apis::models_api::list_models(&self.cfg.get_api_config()).await;
        match response {
            Err(e) => Err(SysinspectError::MasterGeneralError(format!("Failed to list models: {e}"))),
            Ok(r) => Ok(r),
        }
    }

    pub async fn model_descr(&self, name: &str) -> Result<syswebclient::models::ModelResponse, SysinspectError> {
        if self.sid.is_empty() {
            return Err(SysinspectError::MasterGeneralError("Client is not authenticated".to_string()));
        }

        let response = syswebclient::apis::models_api::get_model_details(&self.cfg.get_api_config(), name).await;
        match response {
            Err(e) => Err(SysinspectError::MasterGeneralError(format!("Failed to get model details: {e}"))),
            Ok(r) => Ok(r),
        }
    }
}
