use crate::{master::RotationCommandPayload, registry::mkb::MinionsKeyRegistry};
use chrono::{Duration as ChronoDuration, Utc};
use libcommon::SysinspectError;
use libsysinspect::{
    cfg::mmconf::MasterConfig,
    rsa::rotation::{RotationActor, RsaTransportRotator},
    transport::{
        TransportPeerState, TransportStore,
        secure_bootstrap::{SecureBootstrapDiagnostics, SecureBootstrapSession},
        secure_channel::{SecureChannel, SecurePeerRole},
    },
};
use libsysproto::{
    MasterMessage, MinionMessage, ProtoConversion,
    rqtypes::RequestType,
    secure::{SECURE_SUPPORTED_PROTOCOL_VERSIONS, SecureBootstrapHello, SecureFrame, SecureSessionBinding},
};
use rsa::RsaPublicKey;
use std::{
    collections::{HashMap, HashSet},
    time::{Duration as StdDuration, Instant},
};

const BOOTSTRAP_MALFORMED_WINDOW: StdDuration = StdDuration::from_secs(30);
const BOOTSTRAP_REPLAY_WINDOW: StdDuration = StdDuration::from_secs(300);

/// One active peer session bound to a minion identifier and channel state.
#[derive(Debug)]
pub(crate) struct PeerConnection {
    minion_id: String,
    channel: SecureChannel,
}

/// Decoded inbound peer frames after bootstrap and transport checks.
#[derive(Debug)]
pub(crate) enum IncomingFrame {
    Forward(Vec<u8>),
    Reply(Vec<u8>),
}

/// Outbound peer frames, either one-off replies or broadcast messages.
#[derive(Debug)]
pub(crate) enum OutgoingFrame {
    Broadcast(Box<MasterMessage>),
    Direct(Vec<u8>),
}

/// Stateful master-side transport protocol manager for bootstrap, replay, and peer channel tracking.
#[derive(Debug, Default)]
pub(crate) struct PeerTransport {
    peers: HashMap<String, PeerConnection>,
    plaintext_peers: HashSet<String>,
    bootstrap_failures: HashMap<String, (Instant, u32)>,
    bootstrap_replay_cache: HashMap<String, Instant>,
}

impl PeerTransport {
    /// Create empty peer transport state for a fresh master instance.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Mark a peer as allowed to receive the one-shot plaintext registration reply path.
    pub(crate) fn allow_plaintext(&mut self, peer_addr: &str) {
        self.plaintext_peers.insert(peer_addr.to_string());
    }

    /// Remove all transport state associated with one peer connection.
    pub(crate) fn remove_peer(&mut self, peer_addr: &str) {
        self.peers.remove(peer_addr);
        self.plaintext_peers.remove(peer_addr);
    }

    /// Return whether this peer may receive a broadcast frame right now.
    pub(crate) fn can_receive_broadcast(&self, peer_addr: &str) -> bool {
        Self::can_receive_broadcast_state(self.peers.contains_key(peer_addr), self.plaintext_peers.contains(peer_addr))
    }

    /// Return whether this connection may receive plaintext broadcast traffic before bootstrap completes.
    pub(crate) fn can_receive_broadcast_state(has_session: bool, plaintext_allowed: bool) -> bool {
        has_session || plaintext_allowed
    }

    /// Build a plaintext bootstrap diagnostic when a peer speaks framed transport to a plaintext-only path.
    pub(crate) fn bootstrap_diag(&mut self, peer_addr: &str, data: &[u8]) -> Option<Vec<u8>> {
        Self::bootstrap_diag_with_state(&mut self.bootstrap_failures, peer_addr, data)
    }

    /// Build a plaintext bootstrap diagnostic from shared malformed-attempt state.
    pub(crate) fn bootstrap_diag_with_state(failures: &mut HashMap<String, (Instant, u32)>, peer_addr: &str, data: &[u8]) -> Option<Vec<u8>> {
        let peer_key = Self::rate_limit_key(peer_addr);
        match serde_json::from_slice::<SecureFrame>(data) {
            Ok(SecureFrame::BootstrapHello(_) | SecureFrame::BootstrapAck(_) | SecureFrame::Data(_)) => {
                failures.remove(&peer_key);
                serde_json::to_vec(&SecureBootstrapDiagnostics::unsupported_version("Secure transport is not enabled on this master yet")).ok()
            }
            Err(_) if std::str::from_utf8(data).ok().map(|text| text.contains("\"kind\"")).unwrap_or(false) => {
                serde_json::to_vec(&Self::malformed_diag(failures, &peer_key)).ok()
            }
            _ => None,
        }
    }

    /// Return a plaintext diagnostic when a minion sends normal protocol traffic before bootstrap.
    pub(crate) fn plaintext_diag(data: &[u8]) -> Option<Vec<u8>> {
        match serde_json::from_slice::<MinionMessage>(data) {
            Ok(req) if matches!(req.req_type(), RequestType::Add) => None,
            Ok(_) => serde_json::to_vec(&SecureBootstrapDiagnostics::unsupported_version(
                "Plaintext minion traffic is not allowed; secure bootstrap is required",
            ))
            .ok(),
            Err(_) => None,
        }
    }

    /// Encode one outbound message for a peer, sealing it when a session already exists.
    pub(crate) fn encode_message(&mut self, peer_addr: &str, msg: &MasterMessage) -> Result<Vec<u8>, SysinspectError> {
        if let Some(peer) = self.peers.get_mut(peer_addr) {
            return peer.channel.seal(msg);
        }
        msg.sendable()
    }

    /// Decode one inbound raw frame, handling bootstrap establishment and steady-state decryption.
    pub(crate) fn decode_frame(
        &mut self, peer_addr: &str, raw: &[u8], cfg: &MasterConfig, mkr: &mut MinionsKeyRegistry,
    ) -> Result<IncomingFrame, SysinspectError> {
        if self.peers.contains_key(peer_addr) {
            let decoded = self
                .peers
                .get_mut(peer_addr)
                .ok_or_else(|| SysinspectError::ProtoError(format!("Peer transport state for {peer_addr} disappeared")))?
                .channel
                .open_bytes(raw);
            if decoded.is_err() {
                log::warn!("Session for peer {} became invalid; dropping channel state", peer_addr);
                self.peers.remove(peer_addr);
            }
            return decoded.map(IncomingFrame::Forward);
        }
        if let Ok(SecureFrame::BootstrapHello(hello)) = serde_json::from_slice::<SecureFrame>(raw) {
            return self.accept_bootstrap(peer_addr, &hello, cfg, mkr).map(IncomingFrame::Reply);
        }
        if let Some(diag) = self.bootstrap_diag(peer_addr, raw) {
            return Ok(IncomingFrame::Reply(diag));
        }
        if let Some(diag) = Self::plaintext_diag(raw) {
            return Ok(IncomingFrame::Reply(diag));
        }
        match serde_json::from_slice::<MinionMessage>(raw) {
            Ok(req) if matches!(req.req_type(), RequestType::Add) => Ok(IncomingFrame::Forward(raw.to_vec())),
            Ok(_) => Ok(IncomingFrame::Reply(
                serde_json::to_vec(&SecureBootstrapDiagnostics::unsupported_version(
                    "Plaintext minion traffic is not allowed; secure bootstrap is required",
                ))
                .map_err(SysinspectError::from)?,
            )),
            Err(_) => Err(SysinspectError::ProtoError("Unsupported pre-bootstrap peer traffic".to_string())),
        }
    }

    /// Accept a bootstrap hello from a registered minion and store the resulting session for that peer.
    pub(crate) fn accept_bootstrap(
        &mut self, peer_addr: &str, hello: &SecureBootstrapHello, cfg: &MasterConfig, mkr: &mut MinionsKeyRegistry,
    ) -> Result<Vec<u8>, SysinspectError> {
        let now = Instant::now();
        if hello.supported_versions.is_empty() {
            return serde_json::to_vec(&SecureBootstrapDiagnostics::unsupported_version(
                "Secure bootstrap hello did not advertise any supported protocol versions",
            ))
            .map_err(SysinspectError::from);
        }
        if !hello.supported_versions.iter().any(|version| SECURE_SUPPORTED_PROTOCOL_VERSIONS.contains(version)) {
            return serde_json::to_vec(&SecureBootstrapDiagnostics::unsupported_version(format!(
                "No common secure transport protocol version exists between peer {:?} and local {:?}",
                hello.supported_versions, SECURE_SUPPORTED_PROTOCOL_VERSIONS
            )))
            .map_err(SysinspectError::from);
        }
        if self.peers.iter().any(|(addr, peer)| addr != peer_addr && peer.minion_id == hello.binding.minion_id) {
            log::warn!("Rejecting duplicate bootstrap for minion {} from {}", hello.binding.minion_id, peer_addr);
            return serde_json::to_vec(&SecureBootstrapDiagnostics::duplicate_session(format!(
                "Secure session for {} already exists",
                hello.binding.minion_id
            )))
            .map_err(SysinspectError::from);
        }
        if let Some(message) = Self::bootstrap_precheck(&mut self.bootstrap_replay_cache, &hello.binding, now) {
            log::warn!("Rejecting bootstrap for minion {} from {}: {}", hello.binding.minion_id, peer_addr, message);
            return serde_json::to_vec(&SecureBootstrapDiagnostics::replay_rejected(message)).map_err(SysinspectError::from);
        }
        let mut state = TransportStore::for_master_minion(cfg, &hello.binding.minion_id)?
            .load()?
            .ok_or_else(|| SysinspectError::ProtoError(format!("No managed transport state exists for {}", hello.binding.minion_id)))?;
        let master_prk = mkr.master_private_key()?;
        let minion_pbk = mkr.minion_public_key(&hello.binding.minion_id)?;
        let (bootstrap, ack) = SecureBootstrapSession::accept(
            &state,
            hello,
            &master_prk,
            &minion_pbk,
            None,
            state.active_key_id.clone().or_else(|| state.last_key_id.clone()),
            None,
        )?;
        Self::record_bootstrap_replay(&mut self.bootstrap_replay_cache, &hello.binding, now);

        Self::promote_bootstrap_key(cfg, mkr, &mut state, hello, bootstrap.key_id())?;
        TransportStore::for_master_minion(cfg, &hello.binding.minion_id)?.save(&state)?;
        log::info!(
            "Session established for minion {} from {} using key {} and protocol v{}",
            hello.binding.minion_id,
            peer_addr,
            bootstrap.key_id(),
            hello.binding.protocol_version
        );
        self.peers.insert(
            peer_addr.to_string(),
            PeerConnection { minion_id: hello.binding.minion_id.clone(), channel: SecureChannel::new(SecurePeerRole::Master, &bootstrap)? },
        );
        serde_json::to_vec(&ack).map_err(SysinspectError::from)
    }

    /// Return the replay-cache key for one bootstrap opening attempt.
    pub(crate) fn replay_cache_key(binding: &SecureSessionBinding) -> String {
        format!("{}:{}:{}", binding.minion_id, binding.connection_id, binding.client_nonce)
    }

    /// Normalize peer address for rate-limiting so reconnects from new source ports cannot evade limits.
    pub(crate) fn rate_limit_key(peer_addr: &str) -> String {
        peer_addr.parse::<std::net::SocketAddr>().map(|addr| addr.ip().to_string()).unwrap_or_else(|_| peer_addr.to_string())
    }

    /// Reject stale or already-seen bootstrap openings before any expensive cryptographic work.
    pub(crate) fn bootstrap_precheck(cache: &mut HashMap<String, Instant>, binding: &SecureSessionBinding, now: Instant) -> Option<String> {
        Self::prune_bootstrap_replay_cache(cache, now);
        let current_time = chrono::Utc::now().timestamp();
        let drift = (current_time - binding.timestamp).abs();
        if drift > BOOTSTRAP_REPLAY_WINDOW.as_secs() as i64 {
            return Some(format!("Secure bootstrap timestamp drift {}s exceeds the allowed {}s window", drift, BOOTSTRAP_REPLAY_WINDOW.as_secs()));
        }
        let key = Self::replay_cache_key(binding);
        if cache.contains_key(&key) {
            return Some(format!("Secure bootstrap replay rejected for {}", binding.minion_id));
        }
        None
    }

    /// Record one authenticated bootstrap opening so later duplicates are rejected before RSA decryption.
    pub(crate) fn record_bootstrap_replay(cache: &mut HashMap<String, Instant>, binding: &SecureSessionBinding, now: Instant) {
        Self::prune_bootstrap_replay_cache(cache, now);
        let key = Self::replay_cache_key(binding);
        cache.insert(key, now);
    }

    fn prune_bootstrap_replay_cache(cache: &mut HashMap<String, Instant>, now: Instant) {
        cache.retain(|_, seen_at| now.duration_since(*seen_at) <= BOOTSTRAP_REPLAY_WINDOW);
    }

    /// Apply staged rotation context when the bootstrap key matches the expected promoted key.
    fn promote_bootstrap_key(
        cfg: &MasterConfig, mkr: &mut MinionsKeyRegistry, state: &mut TransportPeerState, hello: &SecureBootstrapHello, bootstrap_key_id: &str,
    ) -> Result<(), SysinspectError> {
        if let Some(context) = state.pending_rotation_context.clone()
            && let Ok(payload) = serde_json::from_str::<RotationCommandPayload>(&context)
            && payload.intent.intent().next_key_id() == bootstrap_key_id
        {
            let overlap = ChronoDuration::seconds(payload.grace_seconds as i64);
            let mut rotator = Self::master_rotator(cfg, mkr, &hello.binding.minion_id)?;
            let master_pbk = RsaPublicKey::from(&mkr.master_private_key()?);
            let _ = rotator.execute_signed_intent_with_overlap(&payload.intent, &master_pbk, overlap)?;
            let _ = rotator.retire_elapsed_keys(Utc::now(), overlap)?;
            *state = rotator.state().clone();
            state.set_pending_rotation_context(None);
            return Ok(());
        }
        state.upsert_key(bootstrap_key_id, libsysinspect::transport::TransportKeyStatus::Active);
        Ok(())
    }

    /// Build the reusable master-side transport rotator for one minion.
    fn master_rotator(cfg: &MasterConfig, mkr: &mut MinionsKeyRegistry, minion_id: &str) -> Result<RsaTransportRotator, SysinspectError> {
        RsaTransportRotator::new(
            RotationActor::Master,
            TransportStore::for_master_minion(cfg, minion_id)?,
            minion_id,
            &libsysinspect::rsa::keys::get_fingerprint(&RsaPublicKey::from(&mkr.master_private_key()?))
                .map_err(|err| SysinspectError::RSAError(err.to_string()))?,
            &mkr.get_mn_key_fingerprint(minion_id)?,
            libsysproto::secure::SECURE_PROTOCOL_VERSION,
        )
    }

    /// Rate-limit repeated malformed bootstrap attempts from the same peer before transport is enabled.
    fn malformed_diag(failures: &mut HashMap<String, (Instant, u32)>, peer_key: &str) -> SecureFrame {
        let now = Instant::now();
        let count = match failures.get_mut(peer_key) {
            Some((seen_at, count)) if now.duration_since(*seen_at) <= BOOTSTRAP_MALFORMED_WINDOW => {
                *count += 1;
                *seen_at = now;
                *count
            }
            Some(entry) => {
                *entry = (now, 1);
                1
            }
            None => {
                failures.insert(peer_key.to_string(), (now, 1));
                1
            }
        };

        if count >= 3 {
            return SecureBootstrapDiagnostics::rate_limited("Repeated malformed secure bootstrap frames");
        }
        SecureBootstrapDiagnostics::malformed("Malformed secure bootstrap frame")
    }

    #[cfg(test)]
    pub(crate) fn accept_bootstrap_auth_then_replay_for_test(
        cache: &mut HashMap<String, Instant>, state: &TransportPeerState, hello: &SecureBootstrapHello, master_prk: &rsa::RsaPrivateKey,
        minion_pbk: &rsa::RsaPublicKey, now: Instant,
    ) -> Result<SecureFrame, SysinspectError> {
        if let Some(message) = Self::bootstrap_precheck(cache, &hello.binding, now) {
            return Ok(SecureBootstrapDiagnostics::replay_rejected(message));
        }
        let (_, ack) = SecureBootstrapSession::accept(
            state,
            hello,
            master_prk,
            minion_pbk,
            None,
            state.active_key_id.clone().or_else(|| state.last_key_id.clone()),
            None,
        )?;
        Self::record_bootstrap_replay(cache, &hello.binding, now);
        Ok(ack)
    }
}
