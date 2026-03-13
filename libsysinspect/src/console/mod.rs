use base64::{Engine, engine::general_purpose::STANDARD};
use libcommon::SysinspectError;
use rsa::RsaPublicKey;
use serde::{Deserialize, Serialize};
use sodiumoxide::crypto::secretbox::{self, Key};
use std::fs;

use crate::{
    cfg::mmconf::MasterConfig,
    rsa::keys::{
        RsaKey::{Private, Public},
        decrypt, encrypt, key_from_file, key_to_file, keygen, sign_data, to_pem, verify_sign,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleBootstrap {
    pub client_pubkey: String,
    pub symkey_cipher: String,
    pub symkey_sign: String,
}

impl ConsoleBootstrap {
    pub fn new(cfg: &MasterConfig) -> Result<Self, SysinspectError> {
        let (client_prk, client_pbk) = ensure_console_keypair(cfg)?;
        let master_pbk = load_master_public_key(cfg)?;
        let symkey = secretbox::gen_key();
        let symkey_cipher = encrypt(master_pbk, symkey.0.to_vec())
            .map_err(|_| SysinspectError::RSAError("Failed to encrypt console session key".to_string()))?;
        let symkey_sign = sign_data(client_prk, &symkey.0)
            .map_err(|_| SysinspectError::RSAError("Failed to sign console session key".to_string()))?;

        Ok(Self {
            client_pubkey: to_pem(None, Some(&client_pbk))
                .map_err(|e| SysinspectError::RSAError(e.to_string()))?
                .1
                .unwrap_or_default(),
            symkey_cipher: STANDARD.encode(symkey_cipher),
            symkey_sign: STANDARD.encode(symkey_sign),
        })
    }

    pub fn session_key(&self, cfg: &MasterConfig) -> Result<Key, SysinspectError> {
        let master_prk = match key_from_file(cfg.root_dir().join(crate::cfg::mmconf::CFG_MASTER_KEY_PRI).to_str().unwrap_or_default())? {
            Some(Private(prk)) => prk,
            Some(_) => return Err(SysinspectError::RSAError("Expected master private key".to_string())),
            None => return Err(SysinspectError::RSAError("Master private key not found".to_string())),
        };
        let client_pbk = match crate::rsa::keys::from_pem(None, Some(&self.client_pubkey))
            .map_err(|e| SysinspectError::RSAError(e.to_string()))?
            .1
        {
            Some(pbk) => pbk,
            None => return Err(SysinspectError::RSAError("Client public key not found in bootstrap".to_string())),
        };
        let symkey = decrypt(
            master_prk,
            STANDARD
                .decode(&self.symkey_cipher)
                .map_err(|e| SysinspectError::SerializationError(format!("Failed to decode console session key: {e}")))?,
        )
        .map_err(|_| SysinspectError::RSAError("Failed to decrypt console session key".to_string()))?;
        let symkey_sign = STANDARD
            .decode(&self.symkey_sign)
            .map_err(|e| SysinspectError::SerializationError(format!("Failed to decode console session signature: {e}")))?;

        if !verify_sign(&client_pbk, &symkey, symkey_sign).map_err(|e| SysinspectError::RSAError(e.to_string()))? {
            return Err(SysinspectError::RSAError("Console session signature verification failed".to_string()));
        }

        Key::from_slice(&symkey).ok_or_else(|| SysinspectError::RSAError("Console session key has invalid size".to_string()))
    }
}

pub fn ensure_console_keypair(cfg: &MasterConfig) -> Result<(rsa::RsaPrivateKey, RsaPublicKey), SysinspectError> {
    if cfg.console_privkey().exists() && cfg.console_pubkey().exists() {
        let prk = match key_from_file(cfg.console_privkey().to_str().unwrap_or_default())? {
            Some(Private(prk)) => prk,
            Some(_) => return Err(SysinspectError::RSAError("Expected console private key".to_string())),
            None => return Err(SysinspectError::RSAError("Console private key not found".to_string())),
        };
        let pbk = match key_from_file(cfg.console_pubkey().to_str().unwrap_or_default())? {
            Some(Public(pbk)) => pbk,
            Some(_) => return Err(SysinspectError::RSAError("Expected console public key".to_string())),
            None => return Err(SysinspectError::RSAError("Console public key not found".to_string())),
        };
        return Ok((prk, pbk));
    }

    fs::create_dir_all(cfg.root_dir()).map_err(SysinspectError::IoErr)?;
    let (prk, pbk) = keygen(crate::rsa::keys::DEFAULT_KEY_SIZE).map_err(|e| SysinspectError::RSAError(e.to_string()))?;
    key_to_file(&Private(prk.clone()), cfg.root_dir().to_str().unwrap_or_default(), crate::cfg::mmconf::CFG_CONSOLE_KEY_PRI)?;
    key_to_file(&Public(pbk.clone()), cfg.root_dir().to_str().unwrap_or_default(), crate::cfg::mmconf::CFG_CONSOLE_KEY_PUB)?;
    Ok((prk, pbk))
}

pub fn load_master_public_key(cfg: &MasterConfig) -> Result<RsaPublicKey, SysinspectError> {
    match key_from_file(cfg.root_dir().join(crate::cfg::mmconf::CFG_MASTER_KEY_PUB).to_str().unwrap_or_default())? {
        Some(Public(pbk)) => Ok(pbk),
        Some(_) => Err(SysinspectError::RSAError("Expected master public key".to_string())),
        None => Err(SysinspectError::RSAError("Master public key not found".to_string())),
    }
}
