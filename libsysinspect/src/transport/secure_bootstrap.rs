use super::TransportPeerState;
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::Utc;
use libcommon::SysinspectError;
use libsysproto::secure::{
    SECURE_PROTOCOL_VERSION, SecureBootstrapAck, SecureBootstrapDiagnostic, SecureBootstrapHello, SecureDiagnosticCode, SecureFailureSemantics,
    SecureFrame, SecureRotationMode, SecureSessionBinding,
};
use rsa::{RsaPrivateKey, RsaPublicKey};
use sha2::{Digest, Sha256};
use sodiumoxide::crypto::secretbox::{self, Key};
use std::sync::OnceLock;
use uuid::Uuid;

use crate::rsa::keys::{decrypt, encrypt, get_fingerprint, sign_data, verify_sign};

static SODIUM_INIT: OnceLock<()> = OnceLock::new();

/// Bootstrap session state created while negotiating a secure Master/Minion link.
#[derive(Debug, Clone)]
pub struct SecureBootstrapSession {
    binding: SecureSessionBinding,
    session_key: Key,
    key_id: String,
    session_id: Option<String>,
}

/// Factory for the plaintext bootstrap diagnostics allowed before a secure session exists.
pub struct SecureBootstrapDiagnostics;

impl SecureBootstrapSession {
    /// Build the opening bootstrap frame from trusted transport state on the minion side.
    pub fn open(state: &TransportPeerState, minion_prk: &RsaPrivateKey, master_pbk: &RsaPublicKey) -> Result<(Self, SecureFrame), SysinspectError> {
        sodium_ready()?;
        Self::ready(state)?;
        Self::fingerprint("master", master_pbk, &state.master_rsa_fingerprint)?;
        let key_id = state.active_key_id.clone().or_else(|| state.last_key_id.clone()).unwrap_or_else(|| Uuid::new_v4().to_string());
        let binding = SecureSessionBinding::bootstrap_opening(
            state.minion_id.clone(),
            state.minion_rsa_fingerprint.clone(),
            state.master_rsa_fingerprint.clone(),
            Uuid::new_v4().to_string(),
            Uuid::new_v4().to_string(),
            Utc::now().timestamp(),
        );
        Self::hello(
            binding.clone(),
            state.key_material(&key_id).as_deref().map(|material| Self::derive_session_key(material, &binding)).unwrap_or_else(secretbox::gen_key),
            key_id,
            minion_prk,
            master_pbk,
        )
    }

    /// Validate a bootstrap hello on the master side and return a signed acknowledgement frame.
    pub fn accept(
        state: &TransportPeerState, hello: &SecureBootstrapHello, master_prk: &RsaPrivateKey, minion_pbk: &RsaPublicKey, session_id: Option<String>,
        key_id: Option<String>, rotation: Option<SecureRotationMode>,
    ) -> Result<(Self, SecureFrame), SysinspectError> {
        Self::ready(state)?;
        Self::opening(state, &hello.binding)?;
        Self::fingerprint("minion", minion_pbk, &state.minion_rsa_fingerprint)?;
        Self::fingerprint("master", &RsaPublicKey::from(master_prk), &state.master_rsa_fingerprint)?;
        Self::ack(
            hello.binding.clone(),
            Self::verify_hello(hello, minion_pbk, master_prk)?,
            hello
                .key_id
                .clone()
                .or_else(|| key_id.clone())
                .or_else(|| state.active_key_id.clone())
                .or_else(|| state.last_key_id.clone())
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            master_prk,
            session_id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            rotation.unwrap_or(SecureRotationMode::None),
        )
    }

    /// Verify the master's bootstrap acknowledgement and finalize the negotiated bootstrap session.
    pub fn verify_ack(mut self, state: &TransportPeerState, ack: &SecureBootstrapAck, master_pbk: &RsaPublicKey) -> Result<Self, SysinspectError> {
        Self::ready(state)?;
        Self::fingerprint("master", master_pbk, &state.master_rsa_fingerprint)?;
        Self::accepted(state, &self.binding, &ack.binding)?;
        if ack.session_id.trim().is_empty() {
            return Err(SysinspectError::ProtoError("Secure bootstrap ack has an empty session id".to_string()));
        }
        if ack.key_id.trim().is_empty() {
            return Err(SysinspectError::ProtoError("Secure bootstrap ack has an empty key id".to_string()));
        }
        if !verify_sign(
            master_pbk,
            &Self::ack_material(&ack.binding, &ack.session_id)?,
            STANDARD
                .decode(&ack.binding_signature)
                .map_err(|err| SysinspectError::SerializationError(format!("Failed to decode secure bootstrap ack signature: {err}")))?,
        )
        .map_err(|err| SysinspectError::RSAError(err.to_string()))?
        {
            return Err(SysinspectError::RSAError("Secure bootstrap ack signature verification failed".to_string()));
        }
        self.binding = ack.binding.clone();
        self.key_id = ack.key_id.clone();
        self.session_id = Some(ack.session_id.clone());
        Ok(self)
    }

    /// Return the identity binding attached to this bootstrap session.
    pub fn binding(&self) -> &SecureSessionBinding {
        &self.binding
    }

    /// Return the established session identifier once the bootstrap acknowledgement was accepted.
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Return the transport key identifier associated with this bootstrap attempt.
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// Return the negotiated libsodium session key for the secure channel.
    pub fn session_key(&self) -> &Key {
        &self.session_key
    }

    /// Encode the minion's opening bootstrap frame and keep the local bootstrap state.
    fn hello(
        binding: SecureSessionBinding, session_key: Key, key_id: String, minion_prk: &RsaPrivateKey, master_pbk: &RsaPublicKey,
    ) -> Result<(Self, SecureFrame), SysinspectError> {
        let session_key_cipher = STANDARD.encode(
            encrypt(master_pbk.clone(), session_key.0.to_vec())
                .map_err(|_| SysinspectError::RSAError("Failed to encrypt secure session key for the master".to_string()))?,
        );
        let binding_signature = STANDARD.encode(
            sign_data(minion_prk.clone(), &Self::hello_material(&binding, &session_key_cipher)?)
                .map_err(|_| SysinspectError::RSAError("Failed to sign secure bootstrap binding".to_string()))?,
        );
        Ok((
            Self { binding: binding.clone(), session_key: session_key.clone(), key_id: key_id.clone(), session_id: None },
            SecureFrame::BootstrapHello(SecureBootstrapHello {
                binding: binding.clone(),
                session_key_cipher,
                binding_signature,
                key_id: Some(key_id),
            }),
        ))
    }

    /// Encode the master's bootstrap acknowledgement after the hello was authenticated successfully.
    fn ack(
        mut binding: SecureSessionBinding, session_key: Key, key_id: String, master_prk: &RsaPrivateKey, session_id: String,
        rotation: SecureRotationMode,
    ) -> Result<(Self, SecureFrame), SysinspectError> {
        binding.master_nonce = Uuid::new_v4().to_string();
        let binding_signature = STANDARD.encode(
            sign_data(master_prk.clone(), &Self::ack_material(&binding, &session_id)?)
                .map_err(|_| SysinspectError::RSAError("Failed to sign secure bootstrap acknowledgement".to_string()))?,
        );
        Ok((
            Self { binding: binding.clone(), session_key, key_id: key_id.clone(), session_id: Some(session_id.clone()) },
            SecureFrame::BootstrapAck(SecureBootstrapAck { binding, session_id: session_id.clone(), key_id, rotation, binding_signature }),
        ))
    }

    /// Check that the stored peer state is approved and matches the active protocol version.
    fn ready(state: &TransportPeerState) -> Result<(), SysinspectError> {
        if state.protocol_version != SECURE_PROTOCOL_VERSION {
            return Err(SysinspectError::ProtoError(format!(
                "Secure transport version mismatch in state: expected {}, found {}",
                SECURE_PROTOCOL_VERSION, state.protocol_version
            )));
        }
        if state.approved_at.is_none() {
            return Err(SysinspectError::ProtoError("Secure transport peer is not approved for bootstrap".to_string()));
        }
        Ok(())
    }

    /// Verify that an opening bootstrap binding matches the trusted peer state before accepting it.
    fn opening(state: &TransportPeerState, binding: &SecureSessionBinding) -> Result<(), SysinspectError> {
        if binding.protocol_version != SECURE_PROTOCOL_VERSION {
            return Err(SysinspectError::ProtoError(format!("Unsupported secure bootstrap version {}", binding.protocol_version)));
        }
        if binding.master_nonce.is_empty()
            && binding.minion_id == state.minion_id
            && binding.minion_rsa_fingerprint == state.minion_rsa_fingerprint
            && binding.master_rsa_fingerprint == state.master_rsa_fingerprint
            && !binding.connection_id.trim().is_empty()
            && !binding.client_nonce.trim().is_empty()
        {
            return Ok(());
        }
        Err(SysinspectError::ProtoError("Secure bootstrap hello binding does not match the trusted peer state".to_string()))
    }

    /// Verify that an acknowledgement binding is still tied to the same handshake attempt and peer identities.
    fn accepted(state: &TransportPeerState, opening: &SecureSessionBinding, binding: &SecureSessionBinding) -> Result<(), SysinspectError> {
        if binding.protocol_version != SECURE_PROTOCOL_VERSION {
            return Err(SysinspectError::ProtoError(format!("Unsupported secure bootstrap ack version {}", binding.protocol_version)));
        }
        if binding.master_nonce.trim().is_empty() {
            return Err(SysinspectError::ProtoError("Secure bootstrap ack is missing the master nonce".to_string()));
        }
        if binding.minion_id == state.minion_id
            && binding.minion_rsa_fingerprint == state.minion_rsa_fingerprint
            && binding.master_rsa_fingerprint == state.master_rsa_fingerprint
            && binding.connection_id == opening.connection_id
            && binding.client_nonce == opening.client_nonce
        {
            return Ok(());
        }
        Err(SysinspectError::ProtoError("Secure bootstrap ack does not match the opening handshake binding".to_string()))
    }

    /// Decrypt and authenticate the opening bootstrap frame sent by the minion.
    fn verify_hello(hello: &SecureBootstrapHello, minion_pbk: &RsaPublicKey, master_prk: &RsaPrivateKey) -> Result<Key, SysinspectError> {
        if !verify_sign(
            minion_pbk,
            &Self::hello_material(&hello.binding, &hello.session_key_cipher)?,
            STANDARD
                .decode(&hello.binding_signature)
                .map_err(|err| SysinspectError::SerializationError(format!("Failed to decode secure bootstrap signature: {err}")))?,
        )
        .map_err(|err| SysinspectError::RSAError(err.to_string()))?
        {
            return Err(SysinspectError::RSAError("Secure bootstrap hello signature verification failed".to_string()));
        }
        let session_key = Self::key(&hello.session_key_cipher, master_prk)?;
        Ok(session_key)
    }

    /// Decrypt the RSA-wrapped libsodium session key from the opening bootstrap frame.
    fn key(cipher: &str, master_prk: &RsaPrivateKey) -> Result<Key, SysinspectError> {
        Key::from_slice(
            &decrypt(
                master_prk.clone(),
                STANDARD
                    .decode(cipher)
                    .map_err(|err| SysinspectError::SerializationError(format!("Failed to decode secure bootstrap session key: {err}")))?,
            )
            .map_err(|_| SysinspectError::RSAError("Failed to decrypt secure bootstrap session key".to_string()))?,
        )
        .ok_or_else(|| SysinspectError::RSAError("Secure bootstrap session key has invalid size".to_string()))
    }

    /// Derive a fresh bootstrap session key from persisted transport material and the opening handshake tuple.
    /// Derive a per-bootstrap session key from persisted transport material and the unique opening binding.
    fn derive_session_key(material: &[u8], binding: &SecureSessionBinding) -> Key {
        let mut digest = Sha256::new();
        digest.update(b"sysinspect-secure-bootstrap");
        digest.update(material);
        digest.update(binding.minion_id.as_bytes());
        digest.update(binding.minion_rsa_fingerprint.as_bytes());
        digest.update(binding.master_rsa_fingerprint.as_bytes());
        digest.update(binding.connection_id.as_bytes());
        digest.update(binding.client_nonce.as_bytes());
        digest.update(binding.protocol_version.to_be_bytes());
        Key::from_slice(&digest.finalize()).unwrap_or_else(secretbox::gen_key)
    }

    /// Build the signed material for a bootstrap hello from the binding and ciphered session key bytes.
    fn hello_material(binding: &SecureSessionBinding, session_key_cipher: &str) -> Result<Vec<u8>, SysinspectError> {
        Self::material(binding, Some(session_key_cipher.as_bytes()), None)
    }

    /// Build the signed material for a bootstrap acknowledgement from the binding and session id.
    fn ack_material(binding: &SecureSessionBinding, session_id: &str) -> Result<Vec<u8>, SysinspectError> {
        Self::material(binding, None, Some(session_id))
    }

    /// Serialize the binding and append the extra authenticated bootstrap material used for signatures.
    fn material(binding: &SecureSessionBinding, session_key: Option<&[u8]>, session_id: Option<&str>) -> Result<Vec<u8>, SysinspectError> {
        serde_json::to_vec(binding)
            .map(|mut out| {
                if let Some(chunk) = session_key {
                    out.extend_from_slice(chunk);
                }
                if let Some(chunk) = session_id {
                    out.extend_from_slice(chunk.as_bytes());
                }
                out
            })
            .map_err(|err| SysinspectError::SerializationError(format!("Failed to serialise secure bootstrap material: {err}")))
    }

    /// Verify that the presented RSA public key fingerprint matches the trusted transport state.
    fn fingerprint(label: &str, pbk: &RsaPublicKey, expected: &str) -> Result<(), SysinspectError> {
        if get_fingerprint(pbk).map_err(|err| SysinspectError::RSAError(err.to_string()))? == expected {
            return Ok(());
        }
        Err(SysinspectError::ProtoError(format!("Trusted {label} fingerprint does not match the secure transport state")))
    }
}

impl SecureBootstrapDiagnostics {
    /// Build a diagnostic for peers that speak an unsupported secure transport version.
    pub fn unsupported_version(message: impl Into<String>) -> SecureFrame {
        Self::frame(SecureDiagnosticCode::UnsupportedVersion, message.into(), true, false)
    }

    /// Build a diagnostic for a bootstrap attempt that was rejected after validation.
    pub fn bootstrap_rejected(message: impl Into<String>) -> SecureFrame {
        Self::frame(SecureDiagnosticCode::BootstrapRejected, message.into(), false, false)
    }

    /// Build a diagnostic for a replayed or otherwise non-fresh bootstrap attempt.
    pub fn replay_rejected(message: impl Into<String>) -> SecureFrame {
        Self::frame(SecureDiagnosticCode::ReplayRejected, message.into(), true, true)
    }

    /// Build a diagnostic for a bootstrap peer that has been rate-limited.
    pub fn rate_limited(message: impl Into<String>) -> SecureFrame {
        Self::frame(SecureDiagnosticCode::RateLimited, message.into(), true, true)
    }

    /// Build a diagnostic for malformed bootstrap input received before a secure session exists.
    pub fn malformed(message: impl Into<String>) -> SecureFrame {
        Self::frame(SecureDiagnosticCode::MalformedFrame, message.into(), false, true)
    }

    /// Build a diagnostic for a peer that attempts to open a duplicate active secure session.
    pub fn duplicate_session(message: impl Into<String>) -> SecureFrame {
        Self::frame(SecureDiagnosticCode::DuplicateSession, message.into(), true, false)
    }

    /// Wrap a plaintext bootstrap diagnostic in the transport frame enum.
    fn frame(code: SecureDiagnosticCode, message: String, retryable: bool, rate_limit: bool) -> SecureFrame {
        SecureFrame::BootstrapDiagnostic(SecureBootstrapDiagnostic {
            code,
            message,
            failure: SecureFailureSemantics::diagnostic(retryable, rate_limit),
        })
    }
}

/// Initialise libsodium exactly once before generating or validating bootstrap session material.
fn sodium_ready() -> Result<(), SysinspectError> {
    if SODIUM_INIT.get().is_none() {
        if sodiumoxide::init().is_err() {
            return Err(SysinspectError::ConfigError("Failed to initialise libsodium".to_string()));
        }
        let _ = SODIUM_INIT.set(());
    }
    Ok(())
}
