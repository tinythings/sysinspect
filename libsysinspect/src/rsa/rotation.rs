use chrono::{DateTime, Duration, Utc};
use libcommon::SysinspectError;
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::rsa::keys::{get_fingerprint, sign_data, verify_sign};
use crate::transport::{TransportKeyStatus, TransportPeerState, TransportRotationStatus, TransportStore};
use base64::{Engine, engine::general_purpose::STANDARD};
use sodiumoxide::crypto::secretbox;

#[cfg(test)]
#[path = "rotation_ut.rs"]
mod rotation_ut;

/// Role of the process performing rotation orchestration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationActor {
    Master,
    Minion,
}

/// Planned key-rotation operation for one master/minion relationship.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotationPlan {
    minion_id: String,
    previous_key_id: Option<String>,
    next_key_id: String,
    next_key_fingerprint: String,
    reason: String,
    requested_at: DateTime<Utc>,
}

impl RotationPlan {
    pub fn minion_id(&self) -> &str {
        &self.minion_id
    }

    pub fn previous_key_id(&self) -> Option<&str> {
        self.previous_key_id.as_deref()
    }

    pub fn next_key_id(&self) -> &str {
        &self.next_key_id
    }

    pub fn next_key_fingerprint(&self) -> &str {
        &self.next_key_fingerprint
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    pub fn requested_at(&self) -> DateTime<Utc> {
        self.requested_at
    }
}

/// Result details returned after a successful execute operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotationResult {
    minion_id: String,
    active_key_id: String,
    active_key_fingerprint: String,
    rotated_at: DateTime<Utc>,
    purged_keys: usize,
}

impl RotationResult {
    pub fn minion_id(&self) -> &str {
        &self.minion_id
    }

    pub fn active_key_id(&self) -> &str {
        &self.active_key_id
    }

    pub fn active_key_fingerprint(&self) -> &str {
        &self.active_key_fingerprint
    }

    pub fn rotated_at(&self) -> DateTime<Utc> {
        self.rotated_at
    }

    pub fn purged_keys(&self) -> usize {
        self.purged_keys
    }
}

/// Rollback ticket containing pre-rotation state and success details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotationTicket {
    previous_state: TransportPeerState,
    result: RotationResult,
}

/// Unsigned transport-key rotation intent to be authenticated with the RSA trust anchor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RotationIntent {
    minion_id: String,
    previous_key_id: Option<String>,
    next_key_id: String,
    next_key_fingerprint: String,
    requested_at: DateTime<Utc>,
    reason: String,
}

impl RotationIntent {
    pub fn minion_id(&self) -> &str {
        &self.minion_id
    }

    pub fn previous_key_id(&self) -> Option<&str> {
        self.previous_key_id.as_deref()
    }

    pub fn next_key_id(&self) -> &str {
        &self.next_key_id
    }

    pub fn next_key_fingerprint(&self) -> &str {
        &self.next_key_fingerprint
    }

    pub fn requested_at(&self) -> DateTime<Utc> {
        self.requested_at
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

/// RSA-signed transport-key rotation intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedRotationIntent {
    signer_fingerprint: String,
    intent: RotationIntent,
    signature: String,
}

impl SignedRotationIntent {
    pub fn signer_fingerprint(&self) -> &str {
        &self.signer_fingerprint
    }

    pub fn intent(&self) -> &RotationIntent {
        &self.intent
    }

    pub fn signature(&self) -> &str {
        &self.signature
    }
}

impl RotationTicket {
    pub fn previous_state(&self) -> &TransportPeerState {
        &self.previous_state
    }

    pub fn result(&self) -> &RotationResult {
        &self.result
    }
}

/// Reusable object that coordinates transport-key rotation state transitions.
pub struct RsaTransportRotator {
    actor: RotationActor,
    store: TransportStore,
    state: TransportPeerState,
}

impl RsaTransportRotator {
    /// Create a reusable rotator and load (or initialise) managed transport state.
    pub fn new(
        actor: RotationActor, store: TransportStore, minion_id: &str, master_rsa_fingerprint: &str, minion_rsa_fingerprint: &str,
        protocol_version: u16,
    ) -> Result<Self, SysinspectError> {
        let state = store.ensure_automatic_peer(minion_id, master_rsa_fingerprint, minion_rsa_fingerprint, protocol_version)?;
        Ok(Self { actor, store, state })
    }

    /// Return the actor role for this rotator.
    pub fn actor(&self) -> RotationActor {
        self.actor
    }

    /// Return the current managed transport state snapshot.
    pub fn state(&self) -> &TransportPeerState {
        &self.state
    }

    /// Return the timestamp of the currently active key as a practical "last rotated" value.
    pub fn last_rotated_at(&self) -> Option<DateTime<Utc>> {
        let active_key = self.state.active_key_id.as_ref()?;
        self.state.keys.iter().find(|record| record.key_id == *active_key).and_then(|record| record.activated_at.or(Some(record.created_at)))
    }

    /// Return whether rotation is due according to the active key timestamp and provided interval.
    pub fn rotation_due(&self, interval: Duration, now: DateTime<Utc>) -> bool {
        match self.last_rotated_at() {
            Some(last) => last + interval <= now,
            None => true,
        }
    }

    /// Mark this peer as pending rotation when due and persist the state.
    pub fn queue_if_due(&mut self, interval: Duration, now: DateTime<Utc>) -> Result<bool, SysinspectError> {
        if !self.rotation_due(interval, now) {
            return Ok(false);
        }

        if matches!(self.state.rotation, TransportRotationStatus::Pending) {
            return Ok(true);
        }

        self.state.rotation = TransportRotationStatus::Pending;
        self.state.updated_at = now;
        self.store.save(&self.state)?;
        Ok(true)
    }

    /// Build a rotation plan with a fresh key identifier and deterministic fingerprint.
    pub fn plan(&self, reason: impl Into<String>) -> RotationPlan {
        let next_key_id = format!("trk-{}", Uuid::new_v4());
        RotationPlan {
            minion_id: self.state.minion_id.clone(),
            previous_key_id: self.state.active_key_id.clone(),
            next_key_fingerprint: Self::fingerprint_for_key_id(&next_key_id),
            next_key_id,
            reason: reason.into(),
            requested_at: Utc::now(),
        }
    }

    /// Build an unsigned rotation intent from a plan.
    pub fn intent_from_plan(&self, plan: &RotationPlan) -> Result<RotationIntent, SysinspectError> {
        if plan.minion_id() != self.state.minion_id {
            return Err(SysinspectError::ConfigError(format!(
                "Rotation plan minion id {} does not match managed state {}",
                plan.minion_id(),
                self.state.minion_id
            )));
        }

        Ok(RotationIntent {
            minion_id: plan.minion_id().to_string(),
            previous_key_id: plan.previous_key_id().map(str::to_string),
            next_key_id: plan.next_key_id().to_string(),
            next_key_fingerprint: plan.next_key_fingerprint().to_string(),
            requested_at: plan.requested_at(),
            reason: plan.reason().to_string(),
        })
    }

    /// Sign a rotation intent with the actor private key.
    pub fn sign_intent(&self, intent: &RotationIntent, signer_prk: &RsaPrivateKey) -> Result<SignedRotationIntent, SysinspectError> {
        let signer_fingerprint = get_fingerprint(&RsaPublicKey::from(signer_prk)).map_err(|err| SysinspectError::RSAError(err.to_string()))?;
        let signature = STANDARD
            .encode(sign_data(signer_prk.clone(), &Self::intent_material(intent)?).map_err(|err| SysinspectError::RSAError(err.to_string()))?);

        Ok(SignedRotationIntent { signer_fingerprint, intent: intent.clone(), signature })
    }

    /// Build and sign a rotation intent from one plan.
    pub fn sign_plan(&self, plan: &RotationPlan, signer_prk: &RsaPrivateKey) -> Result<SignedRotationIntent, SysinspectError> {
        self.sign_intent(&self.intent_from_plan(plan)?, signer_prk)
    }

    /// Verify RSA signature and trust-anchor binding for a signed rotation intent.
    pub fn verify_signed_intent(&self, signed: &SignedRotationIntent, signer_pbk: &RsaPublicKey) -> Result<(), SysinspectError> {
        let expected_fingerprint = self.expected_signer_fingerprint();
        let actual_fingerprint = get_fingerprint(signer_pbk).map_err(|err| SysinspectError::RSAError(err.to_string()))?;
        if actual_fingerprint != expected_fingerprint {
            return Err(SysinspectError::RSAError(format!(
                "Signed rotation intent fingerprint mismatch: expected {}, got {}",
                expected_fingerprint, actual_fingerprint
            )));
        }
        if signed.signer_fingerprint() != expected_fingerprint {
            return Err(SysinspectError::RSAError(format!(
                "Signed rotation intent claims signer {} but expected {}",
                signed.signer_fingerprint(),
                expected_fingerprint
            )));
        }

        let intent = signed.intent();
        if intent.minion_id() != self.state.minion_id {
            return Err(SysinspectError::ConfigError(format!(
                "Signed rotation intent minion id {} does not match managed state {}",
                intent.minion_id(),
                self.state.minion_id
            )));
        }
        if intent.next_key_fingerprint() != Self::fingerprint_for_key_id(intent.next_key_id()) {
            return Err(SysinspectError::ConfigError(format!("Signed rotation intent fingerprint for key {} is invalid", intent.next_key_id())));
        }

        let signature = STANDARD
            .decode(signed.signature())
            .map_err(|err| SysinspectError::SerializationError(format!("Failed to decode rotation signature: {err}")))?;
        if !verify_sign(signer_pbk, &Self::intent_material(intent)?, signature).map_err(|err| SysinspectError::RSAError(err.to_string()))? {
            return Err(SysinspectError::RSAError("Signed rotation intent signature verification failed".to_string()));
        }

        Ok(())
    }

    /// Apply a verified signed intent and execute rotation atomically.
    pub fn execute_signed_intent(&mut self, signed: &SignedRotationIntent, signer_pbk: &RsaPublicKey) -> Result<RotationTicket, SysinspectError> {
        self.execute_signed_intent_with_overlap(signed, signer_pbk, Duration::zero())
    }

    /// Apply a verified signed intent and execute rotation atomically while preserving retiring keys for the overlap window.
    pub fn execute_signed_intent_with_overlap(
        &mut self, signed: &SignedRotationIntent, signer_pbk: &RsaPublicKey, overlap_window: Duration,
    ) -> Result<RotationTicket, SysinspectError> {
        self.verify_signed_intent(signed, signer_pbk)?;
        let plan = RotationPlan {
            minion_id: signed.intent().minion_id().to_string(),
            previous_key_id: signed.intent().previous_key_id().map(str::to_string),
            next_key_id: signed.intent().next_key_id().to_string(),
            next_key_fingerprint: signed.intent().next_key_fingerprint().to_string(),
            reason: signed.intent().reason().to_string(),
            requested_at: signed.intent().requested_at(),
        };

        self.execute_with_overlap(&plan, overlap_window)
    }

    /// Execute a planned rotation atomically in state storage and return a rollback ticket.
    pub fn execute(&mut self, plan: &RotationPlan) -> Result<RotationTicket, SysinspectError> {
        self.execute_with_overlap(plan, Duration::zero())
    }

    /// Execute a planned rotation and keep retiring keys during the overlap window.
    pub fn execute_with_overlap(&mut self, plan: &RotationPlan, overlap_window: Duration) -> Result<RotationTicket, SysinspectError> {
        if plan.minion_id() != self.state.minion_id {
            return Err(SysinspectError::ConfigError(format!(
                "Rotation plan minion id {} does not match managed state {}",
                plan.minion_id(),
                self.state.minion_id
            )));
        }

        let previous_state = self.state.clone();
        let rotated_at = Utc::now();

        let mut next = previous_state.clone();
        next.rotation = TransportRotationStatus::InProgress;
        next.updated_at = rotated_at;

        if let Some(active_key) = previous_state.active_key_id.as_deref()
            && active_key != plan.next_key_id()
        {
            next.upsert_key(active_key, TransportKeyStatus::Retiring);
        }

        let new_material = secretbox::gen_key();
        next.upsert_key_with_material(plan.next_key_id(), TransportKeyStatus::Proposed, Some(&new_material.0));
        next.upsert_key_with_material(plan.next_key_id(), TransportKeyStatus::Active, Some(&new_material.0));

        let purged_keys = self.retire_elapsed_keys_inner(&mut next, rotated_at, overlap_window);

        next.rotation = TransportRotationStatus::Idle;
        next.updated_at = rotated_at;

        self.store.save(&next)?;
        self.state = next;

        Ok(RotationTicket {
            previous_state,
            result: RotationResult {
                minion_id: self.state.minion_id.clone(),
                active_key_id: plan.next_key_id().to_string(),
                active_key_fingerprint: plan.next_key_fingerprint().to_string(),
                rotated_at,
                purged_keys,
            },
        })
    }

    /// Retire and purge keys that have exceeded the overlap window.
    pub fn retire_elapsed_keys(&mut self, now: DateTime<Utc>, overlap_window: Duration) -> Result<usize, SysinspectError> {
        let mut next = self.state.clone();
        let purged = self.retire_elapsed_keys_inner(&mut next, now, overlap_window);
        next.updated_at = now;
        self.store.save(&next)?;
        self.state = next;
        Ok(purged)
    }

    /// Restore pre-rotation state from a rollback ticket.
    pub fn rollback(&mut self, ticket: &RotationTicket) -> Result<TransportPeerState, SysinspectError> {
        self.store.save(ticket.previous_state())?;
        self.state = ticket.previous_state().clone();
        Ok(self.state.clone())
    }

    /// Reconcile with the expected active key fingerprint and queue rotation when mismatched.
    pub fn reconcile_required_fingerprint(&mut self, required_fingerprint: &str, now: DateTime<Utc>) -> Result<bool, SysinspectError> {
        let local_fingerprint = self.state.active_key_id.as_ref().map(|key_id| Self::fingerprint_for_key_id(key_id)).unwrap_or_default();

        if local_fingerprint == required_fingerprint {
            return Ok(false);
        }

        self.state.rotation = TransportRotationStatus::Pending;
        self.state.updated_at = now;
        self.store.save(&self.state)?;
        Ok(true)
    }

    /// Build a deterministic key fingerprint string from a key id.
    pub fn fingerprint_for_key_id(key_id: &str) -> String {
        hex::encode(Sha256::digest(key_id.as_bytes()))
    }

    fn expected_signer_fingerprint(&self) -> &str {
        match self.actor {
            RotationActor::Master => &self.state.master_rsa_fingerprint,
            RotationActor::Minion => &self.state.master_rsa_fingerprint,
        }
    }

    fn intent_material(intent: &RotationIntent) -> Result<Vec<u8>, SysinspectError> {
        serde_json::to_vec(intent).map_err(|err| SysinspectError::SerializationError(format!("Failed to serialize rotation intent: {err}")))
    }

    fn retire_elapsed_keys_inner(&self, state: &mut TransportPeerState, now: DateTime<Utc>, overlap_window: Duration) -> usize {
        if overlap_window <= Duration::zero() {
            let before = state.keys.len();
            let active = state.active_key_id.clone().unwrap_or_default();
            state.keys.retain(|record| record.key_id == active);
            return before.saturating_sub(state.keys.len());
        }

        let active_key = state.active_key_id.clone().unwrap_or_default();
        for key in state.keys.iter_mut() {
            if key.key_id == active_key {
                continue;
            }

            if matches!(key.status, TransportKeyStatus::Retiring) {
                let base = key.activated_at.unwrap_or(key.created_at);
                if base + overlap_window <= now {
                    key.status = TransportKeyStatus::Retired;
                    key.retired_at = Some(now);
                }
            }
        }

        let before = state.keys.len();
        state.keys.retain(|record| !matches!(record.status, TransportKeyStatus::Retired));
        before.saturating_sub(state.keys.len())
    }
}
