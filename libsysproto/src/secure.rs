use serde::{Deserialize, Serialize};

/// Version of the planned secure Master/Minion transport protocol.
pub const SECURE_PROTOCOL_VERSION: u16 = 1;

/// Master/Minion transport goals fixed by Phase 1 decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecureTransportGoals {
    /// Master/Minion transport must not depend on TLS.
    pub no_tls_dependency: bool,
    /// Transport must not depend on DNS.
    pub no_dns_dependency: bool,
    /// Reconnects over unstable links must be a first-class behavior.
    pub reconnect_tolerant: bool,
    /// Steady-state frames must be bounded and authenticated.
    pub bounded_frames: bool,
    /// Replayed secure frames must be rejected.
    pub replay_protection: bool,
    /// Rotation must be supported explicitly.
    pub explicit_rotation: bool,
    /// Only the minimum bootstrap metadata may remain plaintext.
    pub minimal_plaintext_bootstrap: bool,
    /// Non-bootstrap plaintext is rejected once secure framing exists.
    pub reject_non_bootstrap_plaintext: bool,
    /// Only one active secure session may exist for a minion at a time.
    pub single_active_session_per_minion: bool,
}

impl SecureTransportGoals {
    /// Return the fixed Phase 1 transport-goal set for secure Master/Minion communication.
    pub fn master_minion() -> Self {
        Self {
            no_tls_dependency: true,
            no_dns_dependency: true,
            reconnect_tolerant: true,
            bounded_frames: true,
            replay_protection: true,
            explicit_rotation: true,
            minimal_plaintext_bootstrap: true,
            reject_non_bootstrap_plaintext: true,
            single_active_session_per_minion: true,
        }
    }
}

/// Identity binding that ties a secure session to both peers and one connection attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecureSessionBinding {
    /// Registered minion identity.
    pub minion_id: String,
    /// Fingerprint of the minion RSA identity already registered on the master.
    pub minion_rsa_fingerprint: String,
    /// Fingerprint of the master's RSA identity already trusted by the minion.
    pub master_rsa_fingerprint: String,
    /// Secure transport protocol version.
    pub protocol_version: u16,
    /// Per-connection identifier generated for each new handshake attempt.
    pub connection_id: String,
    /// Fresh randomness from the minion side.
    pub client_nonce: String,
    /// Fresh randomness from the master side. Empty in the first bootstrap frame.
    pub master_nonce: String,
}

impl SecureSessionBinding {
    /// Build the opening binding sent by the minion before the master nonce is known.
    pub fn bootstrap_opening(
        minion_id: String, minion_rsa_fingerprint: String, master_rsa_fingerprint: String, connection_id: String, client_nonce: String,
    ) -> Self {
        Self {
            minion_id,
            minion_rsa_fingerprint,
            master_rsa_fingerprint,
            protocol_version: SECURE_PROTOCOL_VERSION,
            connection_id,
            client_nonce,
            master_nonce: String::new(),
        }
    }
}

/// Plaintext diagnostic codes permitted before a secure session exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecureDiagnosticCode {
    UnsupportedVersion,
    BootstrapRejected,
    ReplayRejected,
    RateLimited,
    MalformedFrame,
    DuplicateSession,
}

/// Rotation mode attached to bootstrap acknowledgements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecureRotationMode {
    None,
    Rekey,
    Reregister,
}

/// Failure behavior used by the secure transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecureFailureSemantics {
    /// Whether the peer may retry without operator action.
    pub retryable: bool,
    /// Whether the receiver should disconnect immediately after emitting the diagnostic.
    pub disconnect: bool,
    /// Whether the event should contribute to peer-side rate limiting.
    pub rate_limit: bool,
}

impl SecureFailureSemantics {
    /// Build the failure flags used for a plaintext bootstrap diagnostic.
    pub fn diagnostic(retryable: bool, rate_limit: bool) -> Self {
        Self { retryable, disconnect: true, rate_limit }
    }
}

/// First plaintext bootstrap frame sent by a minion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecureBootstrapHello {
    /// Session binding input that must later be authenticated and echoed.
    pub binding: SecureSessionBinding,
    /// Fresh symmetric session key encrypted to the master's registered RSA key.
    pub session_key_cipher: String,
    /// RSA signature over the bootstrap binding and raw session key.
    pub binding_signature: String,
    /// Optional transport key identifier when reconnecting or rotating.
    pub key_id: Option<String>,
}

/// Plaintext bootstrap acknowledgement returned by the master.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecureBootstrapAck {
    /// Session binding echoed back with the master's nonce filled in.
    pub binding: SecureSessionBinding,
    /// Server-assigned secure session identifier.
    pub session_id: String,
    /// Activated transport key identifier.
    pub key_id: String,
    /// Rotation state communicated during handshake.
    pub rotation: SecureRotationMode,
    /// RSA signature over the completed binding and the accepted session identifier.
    pub binding_signature: String,
}

/// Plaintext bootstrap diagnostic frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecureBootstrapDiagnostic {
    pub code: SecureDiagnosticCode,
    pub message: String,
    pub failure: SecureFailureSemantics,
}

/// Encrypted steady-state frame. Every frame after bootstrap must use this shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecureDataFrame {
    /// Fixed protocol version.
    pub protocol_version: u16,
    /// Established session identifier.
    pub session_id: String,
    /// Transport key identifier active for this frame.
    pub key_id: String,
    /// Monotonic per-direction counter used for replay rejection.
    pub counter: u64,
    /// Libsodium nonce encoded for transport.
    pub nonce: String,
    /// Authenticated encrypted payload.
    pub payload: String,
}

/// Versioned secure Master/Minion frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SecureFrame {
    BootstrapHello(SecureBootstrapHello),
    BootstrapAck(SecureBootstrapAck),
    BootstrapDiagnostic(SecureBootstrapDiagnostic),
    Data(SecureDataFrame),
}

impl SecureFrame {
    /// Only bootstrap frames may remain plaintext.
    pub fn is_plaintext_bootstrap(&self) -> bool {
        matches!(self, Self::BootstrapHello(_) | Self::BootstrapAck(_) | Self::BootstrapDiagnostic(_))
    }

    /// All normal post-bootstrap traffic must be encrypted.
    pub fn requires_established_session(&self) -> bool {
        matches!(self, Self::Data(_))
    }
}

#[cfg(test)]
mod secure_ut;
