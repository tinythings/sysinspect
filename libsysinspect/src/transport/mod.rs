#![doc = include_str!("README.txt")]

pub mod secure_bootstrap;
pub mod secure_channel;

use chrono::{DateTime, Utc};
use libcommon::SysinspectError;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use crate::cfg::mmconf::{CFG_TRANSPORT_MINIONS, CFG_TRANSPORT_STATE, MasterConfig, MinionConfig};

#[cfg(test)]
mod secure_channel_ut;

#[cfg(test)]
mod secure_bootstrap_ut;

#[cfg(test)]
mod transport_ut;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportKeyStatus {
    Proposed,
    Active,
    Retiring,
    Retired,
    Broken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportRotationStatus {
    Idle,
    Pending,
    InProgress,
    RollbackReady,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportProvisioningMode {
    Automatic,
    ExplicitApproval,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportKeyExchangeModel {
    EphemeralSessionKeys,
    PersistedRelationshipKeys,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportKeyRecord {
    pub key_id: String,
    pub status: TransportKeyStatus,
    pub protocol_version: u16,
    pub created_at: DateTime<Utc>,
    pub activated_at: Option<DateTime<Utc>>,
    pub retired_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportPeerState {
    pub minion_id: String,
    pub master_rsa_fingerprint: String,
    pub minion_rsa_fingerprint: String,
    pub protocol_version: u16,
    pub key_exchange: TransportKeyExchangeModel,
    pub provisioning: TransportProvisioningMode,
    pub approved_at: Option<DateTime<Utc>>,
    pub active_key_id: Option<String>,
    pub last_key_id: Option<String>,
    pub last_handshake_at: Option<DateTime<Utc>>,
    pub rotation: TransportRotationStatus,
    pub updated_at: DateTime<Utc>,
    pub keys: Vec<TransportKeyRecord>,
}

impl TransportPeerState {
    pub fn new(minion_id: String, master_rsa_fingerprint: String, minion_rsa_fingerprint: String, protocol_version: u16) -> Self {
        Self {
            minion_id,
            master_rsa_fingerprint,
            minion_rsa_fingerprint,
            protocol_version,
            key_exchange: TransportKeyExchangeModel::EphemeralSessionKeys,
            provisioning: TransportProvisioningMode::Automatic,
            approved_at: Some(Utc::now()),
            active_key_id: None,
            last_key_id: None,
            last_handshake_at: None,
            rotation: TransportRotationStatus::Idle,
            updated_at: Utc::now(),
            keys: Vec::new(),
        }
    }

    pub fn upsert_key(&mut self, key_id: &str, status: TransportKeyStatus) {
        self.last_key_id = Some(key_id.to_string());
        self.updated_at = Utc::now();
        if matches!(status, TransportKeyStatus::Active) {
            self.active_key_id = Some(key_id.to_string());
            self.last_handshake_at = Some(self.updated_at);
        }
        if let Some(key) = self.keys.iter_mut().find(|key| key.key_id.eq(key_id)) {
            key.status = status.clone();
            if matches!(status, TransportKeyStatus::Active) && key.activated_at.is_none() {
                key.activated_at = Some(self.updated_at);
            }
            if matches!(status, TransportKeyStatus::Retired) {
                key.retired_at = Some(self.updated_at);
            }
            return;
        }
        self.keys.push(TransportKeyRecord {
            key_id: key_id.to_string(),
            status: status.clone(),
            protocol_version: self.protocol_version,
            created_at: self.updated_at,
            activated_at: matches!(status, TransportKeyStatus::Active).then_some(self.updated_at),
            retired_at: None,
        });
    }

    pub fn set_provisioning(&mut self, provisioning: TransportProvisioningMode) {
        self.provisioning = provisioning.clone();
        self.updated_at = Utc::now();
        self.approved_at = match provisioning {
            TransportProvisioningMode::Automatic => Some(self.updated_at),
            TransportProvisioningMode::ExplicitApproval => None,
        };
    }

    pub fn approve(&mut self) {
        self.updated_at = Utc::now();
        self.approved_at = Some(self.updated_at);
    }

    pub fn set_key_exchange(&mut self, key_exchange: TransportKeyExchangeModel) {
        self.key_exchange = key_exchange;
        self.updated_at = Utc::now();
    }
}

pub struct TransportStore {
    state_path: PathBuf,
}

impl TransportStore {
    pub fn for_minion(cfg: &MinionConfig) -> Result<Self, SysinspectError> {
        Self::new(cfg.transport_master_root().join(CFG_TRANSPORT_STATE))
    }

    pub fn for_master_minion(cfg: &MasterConfig, minion_id: &str) -> Result<Self, SysinspectError> {
        Self::new(cfg.transport_minions_root().join(Self::safe_peer_dir(minion_id)?).join(CFG_TRANSPORT_STATE))
    }

    pub fn new(state_path: PathBuf) -> Result<Self, SysinspectError> {
        ensure_secure_parent(state_path.parent().ok_or_else(|| {
            SysinspectError::ConfigError(format!("Transport state path {} has no parent", state_path.display()))
        })?)?;
        Ok(Self { state_path })
    }

    pub fn state_path(&self) -> &Path {
        &self.state_path
    }

    pub fn load(&self) -> Result<Option<TransportPeerState>, SysinspectError> {
        if !self.state_path.exists() {
            return Ok(None);
        }
        serde_json::from_str::<TransportPeerState>(&fs::read_to_string(&self.state_path)?)
            .map(Some)
            .map_err(|err| {
                SysinspectError::DeserializationError(format!(
                    "Failed to read transport state from {}: {err}",
                    self.state_path.display()
                ))
            })
    }

    pub fn save(&self, state: &TransportPeerState) -> Result<(), SysinspectError> {
        ensure_secure_parent(self.state_path.parent().unwrap_or_else(|| Path::new(".")))?;
        let tmp = self.state_path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(state)?)?;
        set_file_private(&tmp)?;
        fs::rename(tmp, &self.state_path)?;
        set_file_private(&self.state_path)?;
        Ok(())
    }

    pub fn load_or_init(&self, init: impl FnOnce() -> TransportPeerState) -> Result<TransportPeerState, SysinspectError> {
        self.load().map(|state| state.unwrap_or_else(init))
    }

    pub fn ensure_automatic_peer(
        &self, minion_id: &str, master_rsa_fingerprint: &str, minion_rsa_fingerprint: &str, protocol_version: u16,
    ) -> Result<TransportPeerState, SysinspectError> {
        let mut state = self.load_or_init(|| {
            TransportPeerState::new(
                minion_id.to_string(),
                master_rsa_fingerprint.to_string(),
                minion_rsa_fingerprint.to_string(),
                protocol_version,
            )
        })?;

        state.minion_id = minion_id.to_string();
        state.master_rsa_fingerprint = master_rsa_fingerprint.to_string();
        state.minion_rsa_fingerprint = minion_rsa_fingerprint.to_string();
        state.protocol_version = protocol_version;
        state.key_exchange = TransportKeyExchangeModel::EphemeralSessionKeys;
        if matches!(state.provisioning, TransportProvisioningMode::Automatic) {
            state.approved_at.get_or_insert_with(Utc::now);
            state.updated_at = Utc::now();
        }
        self.save(&state)?;
        Ok(state)
    }

    pub fn require_explicit_approval(
        &self, minion_id: &str, master_rsa_fingerprint: &str, minion_rsa_fingerprint: &str, protocol_version: u16,
    ) -> Result<TransportPeerState, SysinspectError> {
        let mut state = self.ensure_automatic_peer(minion_id, master_rsa_fingerprint, minion_rsa_fingerprint, protocol_version)?;
        state.set_provisioning(TransportProvisioningMode::ExplicitApproval);
        self.save(&state)?;
        Ok(state)
    }

    pub fn approve_peer(&self) -> Result<TransportPeerState, SysinspectError> {
        let mut state = self.load()?.ok_or_else(|| {
            SysinspectError::ConfigError(format!("Transport state does not exist at {}", self.state_path.display()))
        })?;
        state.approve();
        self.save(&state)?;
        Ok(state)
    }

    pub fn safe_peer_dir(peer_id: &str) -> Result<String, SysinspectError> {
        let peer_id = peer_id.trim();
        if peer_id.is_empty()
            || matches!(peer_id, "." | "..")
            || peer_id.contains('/')
            || peer_id.contains('\\')
            || !peer_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        {
            return Err(SysinspectError::ConfigError(format!("Invalid transport peer id: {peer_id}")));
        }
        Ok(peer_id.to_string())
    }
}

pub fn transport_minion_root(root: &Path, minion_id: &str) -> Result<PathBuf, SysinspectError> {
    Ok(root.join(CFG_TRANSPORT_MINIONS).join(TransportStore::safe_peer_dir(minion_id)?))
}

fn ensure_secure_parent(path: &Path) -> Result<(), SysinspectError> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        for entry in path.ancestors().take_while(|ancestor| ancestor.components().all(|component| !matches!(component, Component::RootDir))) {
            if entry.exists() {
                let mut perms = fs::metadata(entry)?.permissions();
                perms.set_mode(0o700);
                fs::set_permissions(entry, perms)?;
            }
        }
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o700);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

fn set_file_private(path: &Path) -> Result<(), SysinspectError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}
