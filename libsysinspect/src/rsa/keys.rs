use rand::rngs::OsRng;
use rsa::{
    pkcs1::{DecodeRsaPrivateKey, DecodeRsaPublicKey, EncodeRsaPrivateKey, EncodeRsaPublicKey},
    pkcs1v15::{Signature, SigningKey, VerifyingKey},
    sha2::{Digest, Sha256},
    signature::SignerMut,
    signature::{Keypair, SignatureEncoding, Verifier},
    Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey,
};
use std::error::Error;

/// Default key size.
pub static DEFAULT_KEY_SIZE: usize = 1048;

/// Generate RSA keys
pub fn keygen(bits: usize) -> Result<(RsaPrivateKey, RsaPublicKey), Box<dyn Error>> {
    let mut rng = OsRng;
    let prk = RsaPrivateKey::new(&mut rng, bits)?;
    let pbk = RsaPublicKey::from(&prk);

    Ok((prk, pbk))
}

/// Serializes RSA private and public keys to PEM format.
pub fn to_pem(prk: &RsaPrivateKey, pbk: &RsaPublicKey) -> Result<(String, String), Box<dyn Error>> {
    Ok((
        pem::encode(&pem::Pem::new("RSA PRIVATE KEY", prk.to_pkcs1_der()?.as_bytes().to_vec())),
        pem::encode(&pem::Pem::new("RSA PUBLIC KEY", pbk.to_pkcs1_der()?.as_bytes().to_vec())),
    ))
}

/// Deserializes RSA private and public keys from PEM format.
pub fn from_pem(prk_pem: &str, pbk_pem: &str) -> Result<(RsaPrivateKey, RsaPublicKey), Box<dyn Error>> {
    Ok((
        RsaPrivateKey::from_pkcs1_der(pem::parse(prk_pem)?.contents())?,
        RsaPublicKey::from_pkcs1_der(pem::parse(pbk_pem)?.contents())?,
    ))
}

/// Sign data with the private key
pub fn sign_data(prk: RsaPrivateKey, data: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut sk = SigningKey::<Sha256>::new(prk);
    let sig = sk.sign(data);
    sk.verifying_key().verify(data, &sig)?;

    Ok((&*sig.to_bytes()).to_vec())
}

/// Verify signature from the pubic key
pub fn verify_sign(pbk: &RsaPublicKey, data: &[u8], sig: Vec<u8>) -> Result<bool, Box<dyn Error>> {
    Ok(VerifyingKey::<Sha256>::new(pbk.clone()).verify(data, &Signature::try_from(sig.as_slice())?).is_ok())
}

/// Get fingerprint of a public key
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
