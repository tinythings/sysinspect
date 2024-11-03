use rand::rngs::OsRng;
use rsa::pkcs1::{DecodeRsaPrivateKey, DecodeRsaPublicKey, EncodeRsaPrivateKey, EncodeRsaPublicKey};
use rsa::{RsaPrivateKey, RsaPublicKey};
use std::error::Error;

/// Default key size.
pub static DEFAULT_KEY_SIZE: usize = 1048;

/// Generate RSA keys
pub fn keygen(bits: usize) -> Result<(RsaPrivateKey, RsaPublicKey), Box<dyn Error>> {
    let mut rng = OsRng;
    let private_key = RsaPrivateKey::new(&mut rng, bits)?;
    let public_key = RsaPublicKey::from(&private_key);

    Ok((private_key, public_key))
}

/// Serializes RSA private and public keys to PEM format.
///
/// # Arguments
///
/// * `private_key` - A reference to the RSA private key.
/// * `public_key` - A reference to the RSA public key.
///
/// # Returns
///
/// A tuple containing the PEM-encoded private and public keys as strings.
pub fn to_pem(private_key: &RsaPrivateKey, public_key: &RsaPublicKey) -> Result<(String, String), Box<dyn Error>> {
    // Serialize private key to PKCS#1 DER
    let private_der = private_key.to_pkcs1_der()?;
    let private_pem = pem::encode(&pem::Pem::new("RSA PRIVATE KEY", private_der.as_bytes().to_vec()));

    // Serialize public key to PKCS#1 DER
    let public_der = public_key.to_pkcs1_der()?;
    let public_pem = pem::encode(&pem::Pem::new("RSA PUBLIC KEY", public_der.as_bytes().to_vec()));

    Ok((private_pem, public_pem))
}

/// Deserializes RSA private and public keys from PEM format.
///
/// # Arguments
///
/// * `private_pem` - A string slice containing the PEM-encoded private key.
/// * `public_pem` - A string slice containing the PEM-encoded public key.
///
/// # Returns
///
/// A tuple containing the deserialized RSA private and public keys.
pub fn from_pem(private_pem: &str, public_pem: &str) -> Result<(RsaPrivateKey, RsaPublicKey), Box<dyn Error>> {
    // Deserialize private key from PEM
    let parsed_private_pem = pem::parse(private_pem)?;
    let private_key = RsaPrivateKey::from_pkcs1_der(parsed_private_pem.contents())?;

    // Deserialize public key from PEM
    let parsed_public_pem = pem::parse(public_pem)?;
    let public_key = RsaPublicKey::from_pkcs1_der(parsed_public_pem.contents())?;

    Ok((private_key, public_key))
}
