use base64::{Engine, engine::general_purpose::STANDARD};
use libcommon::SysinspectError;
use libsysproto::secure::{SECURE_PROTOCOL_VERSION, SecureDataFrame, SecureFrame};
use serde::{Serialize, de::DeserializeOwned};
use sodiumoxide::crypto::secretbox::{self, Key, Nonce};

use super::secure_bootstrap::SecureBootstrapSession;

/// Maximum accepted secure frame size on the wire.
pub const SECURE_MAX_FRAME_SIZE: usize = 1024 * 1024;

/// Maximum accepted plaintext payload size after decryption.
pub const SECURE_MAX_PAYLOAD_SIZE: usize = 512 * 1024;

/// Direction role used to derive unique per-direction nonces from the session counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurePeerRole {
    Master,
    Minion,
}

/// Stateful secure transport channel used after the bootstrap handshake succeeds.
#[derive(Debug, Clone)]
pub struct SecureChannel {
    session_id: String,
    key_id: String,
    key: Key,
    role: SecurePeerRole,
    tx_counter: u64,
    rx_counter: u64,
}

impl SecureChannel {
    /// Create a steady-state secure channel from an accepted bootstrap session.
    pub fn new(role: SecurePeerRole, bootstrap: &SecureBootstrapSession) -> Result<Self, SysinspectError> {
        Ok(Self {
            session_id: bootstrap
                .session_id()
                .ok_or_else(|| SysinspectError::ProtoError("Secure channel requires an established bootstrap session id".to_string()))?
                .to_string(),
            key_id: bootstrap.key_id().to_string(),
            key: bootstrap.session_key().clone(),
            role,
            tx_counter: 0,
            rx_counter: 0,
        })
    }

    /// Return the established secure session identifier used by this channel.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Return the active transport key identifier used by this channel.
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// Seal a serializable payload into a `data` secure frame encoded as JSON bytes.
    pub fn seal<T: Serialize>(&mut self, payload: &T) -> Result<Vec<u8>, SysinspectError> {
        self.seal_bytes(
            &serde_json::to_vec(payload)
                .map_err(|err| SysinspectError::SerializationError(format!("Failed to serialize secure channel payload: {err}")))?,
        )
    }

    /// Seal raw payload bytes into a `data` secure frame encoded as JSON bytes.
    pub fn seal_bytes(&mut self, payload: &[u8]) -> Result<Vec<u8>, SysinspectError> {
        if payload.len() > SECURE_MAX_PAYLOAD_SIZE {
            return Err(SysinspectError::ProtoError(format!("Secure payload exceeds maximum size of {SECURE_MAX_PAYLOAD_SIZE} bytes")));
        }
        self.tx_counter =
            self.tx_counter.checked_add(1).ok_or_else(|| SysinspectError::ProtoError("Secure transmit counter overflow".to_string()))?;
        serde_json::to_vec(&SecureFrame::Data(SecureDataFrame {
            protocol_version: SECURE_PROTOCOL_VERSION,
            session_id: self.session_id.clone(),
            key_id: self.key_id.clone(),
            counter: self.tx_counter,
            nonce: STANDARD.encode(Self::nonce(self.role, self.tx_counter).0),
            payload: STANDARD.encode(secretbox::seal(payload, &Self::nonce(self.role, self.tx_counter), &self.key)),
        }))
        .map_err(|err| SysinspectError::SerializationError(format!("Failed to encode secure data frame: {err}")))
    }

    /// Open a `data` secure frame from JSON bytes and deserialize it to the requested payload type.
    pub fn open<T: DeserializeOwned>(&mut self, frame: &[u8]) -> Result<T, SysinspectError> {
        serde_json::from_slice(&self.open_bytes(frame)?)
            .map_err(|err| SysinspectError::DeserializationError(format!("Failed to decode secure channel payload: {err}")))
    }

    /// Open a `data` secure frame from JSON bytes and return the decrypted raw payload.
    pub fn open_bytes(&mut self, frame: &[u8]) -> Result<Vec<u8>, SysinspectError> {
        if frame.len() > SECURE_MAX_FRAME_SIZE {
            return Err(SysinspectError::ProtoError(format!("Secure frame exceeds maximum size of {SECURE_MAX_FRAME_SIZE} bytes")));
        }
        match serde_json::from_slice::<SecureFrame>(frame)
            .map_err(|err| SysinspectError::DeserializationError(format!("Failed to decode secure frame: {err}")))?
        {
            SecureFrame::Data(data) => self.open_data(data),
            _ => Err(SysinspectError::ProtoError("Expected encrypted secure data frame".to_string())),
        }
    }

    /// Reset the receive counter for a fresh reconnect-driven channel replacement.
    pub fn reset_rx(&mut self) {
        self.rx_counter = 0;
    }

    /// Reset the transmit counter for a fresh reconnect-driven channel replacement.
    pub fn reset_tx(&mut self) {
        self.tx_counter = 0;
    }

    /// Decrypt and validate an incoming secure `data` frame.
    fn open_data(&mut self, frame: SecureDataFrame) -> Result<Vec<u8>, SysinspectError> {
        if frame.protocol_version != SECURE_PROTOCOL_VERSION {
            return Err(SysinspectError::ProtoError(format!("Unsupported secure data frame version {}", frame.protocol_version)));
        }
        if frame.session_id != self.session_id {
            return Err(SysinspectError::ProtoError("Secure data frame session id does not match the active secure channel".to_string()));
        }
        if frame.key_id != self.key_id {
            return Err(SysinspectError::ProtoError("Secure data frame key id does not match the active secure channel".to_string()));
        }
        if frame.counter <= self.rx_counter {
            return Err(SysinspectError::ProtoError(format!("Replay rejected for secure frame counter {}", frame.counter)));
        }
        if frame.counter != self.rx_counter.saturating_add(1) {
            return Err(SysinspectError::ProtoError(format!("Secure frame counter {} is out of sequence after {}", frame.counter, self.rx_counter)));
        }
        let expected_nonce = Self::nonce(Self::peer_role(self.role), frame.counter);
        if STANDARD.encode(expected_nonce.0) != frame.nonce {
            return Err(SysinspectError::ProtoError("Secure data frame nonce does not match the expected counter-derived nonce".to_string()));
        }
        let payload = secretbox::open(
            &STANDARD.decode(&frame.payload).map_err(|err| SysinspectError::SerializationError(format!("Failed to decode secure payload: {err}")))?,
            &expected_nonce,
            &self.key,
        )
        .map_err(|_| SysinspectError::ProtoError("Failed to authenticate or decrypt secure payload".to_string()))?;
        if payload.len() > SECURE_MAX_PAYLOAD_SIZE {
            return Err(SysinspectError::ProtoError(format!("Decrypted secure payload exceeds maximum size of {SECURE_MAX_PAYLOAD_SIZE} bytes")));
        }
        self.rx_counter = frame.counter;
        Ok(payload)
    }

    /// Derive a deterministic nonce from the sender role and monotonic counter.
    fn nonce(role: SecurePeerRole, counter: u64) -> Nonce {
        let mut nonce = [0u8; secretbox::NONCEBYTES];
        nonce[0] = match role {
            SecurePeerRole::Master => 1,
            SecurePeerRole::Minion => 2,
        };
        nonce[secretbox::NONCEBYTES - 8..].copy_from_slice(&counter.to_be_bytes());
        Nonce(nonce)
    }

    /// Return the opposite role used to validate the sender side of an incoming frame.
    fn peer_role(role: SecurePeerRole) -> SecurePeerRole {
        match role {
            SecurePeerRole::Master => SecurePeerRole::Minion,
            SecurePeerRole::Minion => SecurePeerRole::Master,
        }
    }
}
