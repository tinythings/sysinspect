use libcommon::SysinspectError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalMarker {
    pub root: String,
    pub init: String,
}

impl LocalMarker {
    /// Hopstart is when sysminion is started by a master via SSH
    pub fn hopstart(root: &str) -> Self {
        Self { root: root.to_string(), init: "hopstart".to_string() }
    }

    /// Systemd is when sysminion is started by the systemd init system
    pub fn systemd(root: &str) -> Self {
        Self { root: root.to_string(), init: "systemd".to_string() }
    }

    /// BSD init system rc.d
    pub fn rc_d(root: &str) -> Self {
        Self { root: root.to_string(), init: "rc.d".to_string() }
    }

    pub fn to_yaml(&self) -> Result<String, SysinspectError> {
        serde_yaml::to_string(self).map_err(|err| SysinspectError::SerializationError(err.to_string()))
    }

    pub fn from_yaml(data: &str) -> Result<Self, SysinspectError> {
        serde_yaml::from_str::<Self>(data).map_err(|err| SysinspectError::DeserializationError(err.to_string())).and_then(|marker| {
            if marker.root.starts_with('/') && !marker.init.trim().is_empty() {
                Ok(marker)
            } else {
                Err(SysinspectError::ConfigError("Managed install marker is invalid".to_string()))
            }
        })
    }
}
