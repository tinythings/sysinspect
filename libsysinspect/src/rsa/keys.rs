use rand::rngs::OsRng;
use rsa::{
    pkcs1::{DecodeRsaPrivateKey, DecodeRsaPublicKey, EncodeRsaPrivateKey, EncodeRsaPublicKey},
    pkcs1v15::{Signature, SigningKey, VerifyingKey},
    sha2::{Digest, Sha256},
    signature::SignerMut,
    signature::{Keypair, SignatureEncoding, Verifier},
    Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey,
};
use std::{error::Error, fs, io, path::PathBuf};

use crate::SysinspectError;

/// Default key size.
pub static DEFAULT_KEY_SIZE: usize = 1048;

#[allow(clippy::large_enum_variant)]
pub enum RsaKey {
    Private(RsaPrivateKey),
    Public(RsaPublicKey),
}

/// Generate RSA keys
pub fn keygen(bits: usize) -> Result<(RsaPrivateKey, RsaPublicKey), Box<dyn Error + Send + Sync>> {
    let mut rng = OsRng;
    let prk = RsaPrivateKey::new(&mut rng, bits)?;
    let pbk = RsaPublicKey::from(&prk);

    Ok((prk, pbk))
}

/// Serializes RSA private and public keys to PEM format.
pub fn to_pem(
    prk: Option<&RsaPrivateKey>, pbk: Option<&RsaPublicKey>,
) -> Result<(Option<String>, Option<String>), Box<dyn Error + Send + Sync>> {
    Ok((
        if prk.is_some() {
            Some(pem::encode(&pem::Pem::new("RSA PRIVATE KEY", prk.unwrap().to_pkcs1_der()?.as_bytes().to_vec())))
        } else {
            None
        },
        if pbk.is_some() {
            Some(pem::encode(&pem::Pem::new("RSA PUBLIC KEY", pbk.unwrap().to_pkcs1_der()?.as_bytes().to_vec())))
        } else {
            None
        },
    ))
}

/// Deserializes RSA private and public keys from PEM format.
pub fn from_pem(
    prk_pem: Option<&str>, pbk_pem: Option<&str>,
) -> Result<(Option<RsaPrivateKey>, Option<RsaPublicKey>), Box<dyn Error + Send + Sync>> {
    Ok((
        if prk_pem.is_some() {
            Some(RsaPrivateKey::from_pkcs1_der(pem::parse(prk_pem.unwrap_or_default())?.contents())?)
        } else {
            None
        },
        if pbk_pem.is_some() {
            Some(RsaPublicKey::from_pkcs1_der(pem::parse(pbk_pem.unwrap_or_default())?.contents())?)
        } else {
            None
        },
    ))
}

/// Sign data with the private key
pub fn sign_data(prk: RsaPrivateKey, data: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut sk = SigningKey::<Sha256>::new(prk);
    let sig = sk.sign(data);
    sk.verifying_key().verify(data, &sig)?;

    Ok((*sig.to_bytes()).to_vec())
}

/// Verify signature from the pubic key
pub fn verify_sign(pbk: &RsaPublicKey, data: &[u8], sig: Vec<u8>) -> Result<bool, Box<dyn Error>> {
    Ok(VerifyingKey::<Sha256>::new(pbk.clone()).verify(data, &Signature::try_from(sig.as_slice())?).is_ok())
}

/// Get fingerprint of a public key
#[allow(clippy::format_collect)]
pub fn get_fingerprint(pbk: &RsaPublicKey) -> Result<String, Box<dyn Error>> {
    let mut digest = Sha256::new();
    digest.update(pbk.to_pkcs1_der()?.as_bytes());
    Ok(digest.finalize().iter().map(|byte| format!("{:02x}", byte)).collect())
}

// Encrypt data
pub fn encrypt(pbk: RsaPublicKey, data: Vec<u8>) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(pbk.encrypt(&mut rand::thread_rng(), Pkcs1v15Encrypt, &data[..])?)
}

// Decrypt data
pub fn decrypt(prk: RsaPrivateKey, cipher: Vec<u8>) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(prk.decrypt(Pkcs1v15Encrypt, &cipher)?)
}

/// Write private or a public key to a file
pub fn key_to_file(prk: &RsaKey, p: &str, name: &str) -> Result<(), SysinspectError> {
    let p = PathBuf::from(p).join(name);
    if p.exists() {
        return Err(SysinspectError::IoErr(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("File {} already exists", p.to_str().unwrap_or_default()),
        )));
    }

    let mut pem = String::default();
    match prk {
        RsaKey::Private(prk) => {
            if let Ok((prk_pem, _)) = to_pem(Some(prk), None) {
                if let Some(prk_pem) = prk_pem {
                    pem = prk_pem;
                }
            } else {
                return Err(SysinspectError::IoErr(io::Error::new(io::ErrorKind::InvalidData, "Unable to create PEM key")));
            }
        }
        RsaKey::Public(pbk) => {
            if let Ok((_, pbk_pem)) = to_pem(None, Some(pbk)) {
                if let Some(pbk_pem) = pbk_pem {
                    pem = pbk_pem;
                }
            } else {
                return Err(SysinspectError::IoErr(io::Error::new(io::ErrorKind::InvalidData, "Unable to create PEM key")));
            }
        }
    }
    fs::write(&p, pem.as_bytes())?;
    log::debug!("Wrote PEM file as {}", p.to_str().unwrap_or_default());

    Ok(())
}

/// Read private or a public key from a file
pub fn key_from_file(p: &str) -> Result<Option<RsaKey>, SysinspectError> {
    let pth = PathBuf::from(p);
    if !pth.exists() {
        return Err(SysinspectError::IoErr(io::Error::new(io::ErrorKind::NotFound, format!("File {} not found", p))));
    }

    let data = &fs::read_to_string(pth)?;

    if data.contains("RSA PRIVATE KEY") {
        if let Ok((Some(prk), _)) = from_pem(Some(data), None) {
            return Ok(Some(RsaKey::Private(prk)));
        }
    } else if let Ok((_, Some(pbk))) = from_pem(None, Some(data)) {
        return Ok(Some(RsaKey::Public(pbk)));
    }

    Ok(None)
}
