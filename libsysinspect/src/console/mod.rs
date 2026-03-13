//! Encrypted console transport primitives shared by `sysinspect` and `sysmaster`.

use base64::{Engine, engine::general_purpose::STANDARD};
use libcommon::SysinspectError;
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sodiumoxide::crypto::secretbox::{self, Key, Nonce};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use crate::{
    cfg::mmconf::{CFG_CONSOLE_KEY_PRI, CFG_CONSOLE_KEY_PUB, MasterConfig},
    rsa::keys::{
        RsaKey::{Private, Public},
        decrypt, encrypt, key_from_file, key_to_file, keygen, sign_data, to_pem, verify_sign,
    },
};

#[cfg(test)]
mod console_ut;

static SODIUM_INIT: OnceLock<()> = OnceLock::new();

/// RSA-bootstrapped session bootstrap data sent before opening the sealed console payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleBootstrap {
    /// Client console public key in PEM format.
    pub client_pubkey: String,
    /// Session key encrypted to the master's RSA public key.
    pub symkey_cipher: String,
    /// Signature over the raw session key bytes using the client RSA private key.
    pub symkey_sign: String,
}

/// Symmetrically encrypted console frame payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleSealed {
    /// Base64-encoded libsodium nonce.
    pub nonce: String,
    /// Base64-encoded libsodium `secretbox` payload.
    pub payload: String,
}

/// Full console request envelope containing the RSA bootstrap and sealed request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleEnvelope {
    /// Bootstrap data used to derive the symmetric session key.
    pub bootstrap: ConsoleBootstrap,
    /// Encrypted request payload.
    pub sealed: ConsoleSealed,
}

/// Structured console request sent from `sysinspect` to `sysmaster`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleQuery {
    /// Requested model or command URI.
    pub model: String,
    /// Target query string or hostname glob.
    pub query: String,
    /// Optional traits selector expression.
    pub traits: String,
    /// Optional direct minion System Id target.
    pub mid: String,
    /// Optional JSON-encoded context payload.
    pub context: String,
}

/// Structured console response returned by `sysmaster`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleResponse {
    /// Response success flag.
    pub ok: bool,
    /// Human-readable response message or payload.
    pub message: String,
}

/// Ensure the local libsodium state is initialised once for console sealing operations.
fn sodium_ready() -> Result<(), SysinspectError> {
    if SODIUM_INIT.get().is_some() {
        return Ok(());
    }
    if sodiumoxide::init().is_err() {
        return Err(SysinspectError::ConfigError("Failed to initialise libsodium".to_string()));
    }
    let _ = SODIUM_INIT.set(());
    Ok(())
}

fn console_keypair(root: &Path) -> (PathBuf, PathBuf) {
    (root.join(CFG_CONSOLE_KEY_PRI), root.join(CFG_CONSOLE_KEY_PUB))
}

/// Ensure a console RSA keypair exists under the given root and return it.
pub fn ensure_console_keypair(root: &Path) -> Result<(RsaPrivateKey, RsaPublicKey), SysinspectError> {
    let (prk_path, pbk_path) = console_keypair(root);
    if prk_path.exists() && pbk_path.exists() {
        return Ok((load_private_key(&prk_path)?, load_public_key(&pbk_path)?));
    }

    fs::create_dir_all(root).map_err(SysinspectError::IoErr)?;
    let (prk, pbk) = keygen(crate::rsa::keys::DEFAULT_KEY_SIZE).map_err(|e| SysinspectError::RSAError(e.to_string()))?;
    key_to_file(&Private(prk.clone()), root.to_str().unwrap_or_default(), CFG_CONSOLE_KEY_PRI)?;
    key_to_file(&Public(pbk.clone()), root.to_str().unwrap_or_default(), CFG_CONSOLE_KEY_PUB)?;
    Ok((prk, pbk))
}

/// Load the master's public RSA key used for console session bootstrap.
pub fn load_master_public_key(cfg: &MasterConfig) -> Result<RsaPublicKey, SysinspectError> {
    load_public_key(&cfg.root_dir().join(crate::cfg::mmconf::CFG_MASTER_KEY_PUB))
}

/// Load the master's private RSA key used for console session bootstrap.
pub fn load_master_private_key(cfg: &MasterConfig) -> Result<RsaPrivateKey, SysinspectError> {
    load_private_key(&cfg.root_dir().join(crate::cfg::mmconf::CFG_MASTER_KEY_PRI))
}

fn load_private_key(path: &Path) -> Result<RsaPrivateKey, SysinspectError> {
    match key_from_file(path.to_str().unwrap_or_default())? {
        Some(Private(prk)) => Ok(prk),
        Some(_) => Err(SysinspectError::RSAError(format!("Expected private key at {}", path.display()))),
        None => Err(SysinspectError::RSAError(format!("Private key not found at {}", path.display()))),
    }
}

fn load_public_key(path: &Path) -> Result<RsaPublicKey, SysinspectError> {
    match key_from_file(path.to_str().unwrap_or_default())? {
        Some(Public(pbk)) => Ok(pbk),
        Some(_) => Err(SysinspectError::RSAError(format!("Expected public key at {}", path.display()))),
        None => Err(SysinspectError::RSAError(format!("Public key not found at {}", path.display()))),
    }
}

impl ConsoleBootstrap {
    /// Build bootstrap material for a new console session.
    pub fn new(client_prk: &RsaPrivateKey, client_pbk: &RsaPublicKey, master_pbk: &RsaPublicKey, symkey: &Key) -> Result<Self, SysinspectError> {
        Ok(Self {
            client_pubkey: to_pem(None, Some(client_pbk))
                .map_err(|e| SysinspectError::RSAError(e.to_string()))?
                .1
                .unwrap_or_default(),
            symkey_cipher: STANDARD.encode(
                encrypt(master_pbk.clone(), symkey.0.to_vec())
                    .map_err(|_| SysinspectError::RSAError("Failed to encrypt console session key".to_string()))?,
            ),
            symkey_sign: STANDARD.encode(
                sign_data(client_prk.clone(), &symkey.0)
                    .map_err(|_| SysinspectError::RSAError("Failed to sign console session key".to_string()))?,
            ),
        })
    }

    /// Recover and verify the console session key from the bootstrap payload.
    pub fn session_key(&self, master_prk: &RsaPrivateKey) -> Result<(Key, RsaPublicKey), SysinspectError> {
        let client_pbk = crate::rsa::keys::from_pem(None, Some(&self.client_pubkey))
            .map_err(|e| SysinspectError::RSAError(e.to_string()))?
            .1
            .ok_or_else(|| SysinspectError::RSAError("Client public key missing from console bootstrap".to_string()))?;
        let symkey = decrypt(
            master_prk.clone(),
            STANDARD
                .decode(&self.symkey_cipher)
                .map_err(|e| SysinspectError::SerializationError(format!("Failed to decode console session key: {e}")))?,
        )
        .map_err(|_| SysinspectError::RSAError("Failed to decrypt console session key".to_string()))?;
        let signature = STANDARD
            .decode(&self.symkey_sign)
            .map_err(|e| SysinspectError::SerializationError(format!("Failed to decode console session signature: {e}")))?;

        if !verify_sign(&client_pbk, &symkey, signature).map_err(|e| SysinspectError::RSAError(e.to_string()))? {
            return Err(SysinspectError::RSAError("Console session signature verification failed".to_string()));
        }

        Ok((
            Key::from_slice(&symkey).ok_or_else(|| SysinspectError::RSAError("Console session key has invalid size".to_string()))?,
            client_pbk,
        ))
    }
}

impl ConsoleSealed {
    /// Seal a serializable console payload with the given symmetric session key.
    pub fn seal<T: Serialize>(payload: &T, key: &Key) -> Result<Self, SysinspectError> {
        sodium_ready()?;
        let nonce = secretbox::gen_nonce();
        Ok(Self {
            nonce: STANDARD.encode(nonce.0),
            payload: STANDARD.encode(secretbox::seal(
                &serde_json::to_vec(payload).map_err(|e| SysinspectError::SerializationError(e.to_string()))?,
                &nonce,
                key,
            )),
        })
    }

    /// Open a sealed console payload with the given symmetric session key.
    pub fn open<T: DeserializeOwned>(&self, key: &Key) -> Result<T, SysinspectError> {
        sodium_ready()?;
        let nonce = Nonce::from_slice(
            &STANDARD
                .decode(&self.nonce)
                .map_err(|e| SysinspectError::SerializationError(format!("Failed to decode console nonce: {e}")))?,
        )
        .ok_or_else(|| SysinspectError::SerializationError("Console nonce has invalid size".to_string()))?;
        let payload = secretbox::open(
            &STANDARD
                .decode(&self.payload)
                .map_err(|e| SysinspectError::SerializationError(format!("Failed to decode console payload: {e}")))?,
            &nonce,
            key,
        )
        .map_err(|_| SysinspectError::RSAError("Failed to decrypt console payload".to_string()))?;
        serde_json::from_slice(&payload).map_err(|e| SysinspectError::DeserializationError(e.to_string()))
    }
}

/// Check whether the provided client console public key is authorised by the master.
pub fn authorised_console_client(cfg: &MasterConfig, client_pem: &str) -> Result<bool, SysinspectError> {
    if cfg.console_pubkey().exists() && fs::read_to_string(cfg.console_pubkey()).map_err(SysinspectError::IoErr)? == client_pem {
        return Ok(true);
    }

    let root = cfg.console_keys_root();
    if !root.exists() {
        return Ok(false);
    }

    for entry in fs::read_dir(root).map_err(SysinspectError::IoErr)? {
        let path = entry.map_err(SysinspectError::IoErr)?.path();
        if path.is_file() && fs::read_to_string(&path).map_err(SysinspectError::IoErr)? == client_pem {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Build a fully bootstrapped encrypted console request envelope for the given query.
pub fn build_console_query(root: &Path, cfg: &MasterConfig, query: &ConsoleQuery) -> Result<(ConsoleEnvelope, Key), SysinspectError> {
    sodium_ready()?;
    let (client_prk, client_pbk) = ensure_console_keypair(root)?;
    let master_pbk = load_master_public_key(cfg)?;
    let key = secretbox::gen_key();
    Ok((
        ConsoleEnvelope {
            bootstrap: ConsoleBootstrap::new(&client_prk, &client_pbk, &master_pbk, &key)?,
            sealed: ConsoleSealed::seal(query, &key)?,
        },
        key,
    ))
}
