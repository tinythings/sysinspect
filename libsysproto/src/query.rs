use crate::SysinspectError;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Targeting schemes
pub static SCHEME_MODEL: &str = "model://";
pub static SCHEME_COMMAND: &str = "cmd://";

pub mod commands {
    // Start selected offline hopstart-backed minions
    pub const CLUSTER_HOPSTART: &str = "cluster/hopstart";

    // Stop the entire cluster
    pub const CLUSTER_SHUTDOWN: &str = "cluster/shutdown";

    // Sync the entire cluster
    pub const CLUSTER_SYNC: &str = "cluster/sync";

    // Restart the entire cluster
    // TODO: Not implemented yet
    pub const CLUSTER_REBOOT: &str = "cluster/reboot";

    // Rotate RSA/AES on the entire cluster
    pub const CLUSTER_ROTATE: &str = "cluster/rotate";

    // Report transport status for one or more minions
    pub const CLUSTER_TRANSPORT_STATUS: &str = "cluster/transport/status";

    // Remove minion (unregister)
    pub const CLUSTER_REMOVE_MINION: &str = "cluster/minion/remove";

    // Get online minions
    pub const CLUSTER_ONLINE_MINIONS: &str = "cluster/minion/online";

    // Get detailed minion registry information
    pub const CLUSTER_MINION_INFO: &str = "cluster/minion/info";

    // Read recent raw log snapshot from one minion
    pub const CLUSTER_MINION_LOGS: &str = "cluster/minion/logs";

    // Update master-managed static traits on minions
    pub const CLUSTER_TRAITS_UPDATE: &str = "cluster/traits/update";

    // Manage deployment profiles on the master
    pub const CLUSTER_PROFILE: &str = "cluster/profile";

    // Upsert startup inventory / CMDB information for one registered minion
    pub const CLUSTER_CMDB_UPSERT: &str = "cluster/cmdb/upsert";

    // List available models on the master
    pub const CLUSTER_MODELS: &str = "cluster/models";

    // SSH-start one offline hopstart-backed minion
    pub const CLUSTER_MINION_HOPSTART: &str = "cluster/minion/hopstart";

    // Shut down one specific minion
    pub const CLUSTER_MINION_SHUTDOWN: &str = "cluster/minion/shutdown";

    // Force one minion to drop and re-establish its transport connection
    pub const CLUSTER_MINION_RECONNECT: &str = "cluster/minion/reconnect";

    // Force all online minions to reconnect (cluster-wide broadcast)
    pub const CLUSTER_RECONNECT: &str = "cluster/reconnect";

    // Read recent raw log snapshot from the master (standard + error logs)
    pub const CLUSTER_MASTER_LOGS: &str = "cluster/master/logs";

    // Get the module repository index from the master
    pub const CLUSTER_MODULE_INDEX: &str = "cluster/module/index";
}

///
/// Query parser (scheme).
/// It has the following format:
///
/// ```text
/// <model>/[entity]/[state]
/// <model>:[checkbook labels]
/// ```
///
/// If `"entity"` and/or `"state"` are omitted, they are globbed to `"$"` (all).
#[derive(Debug, Clone, Default)]
pub struct MinionQuery {
    src: String,
    entity: Option<String>,
    state: Option<String>,
    scheme: String,
    labels: Option<String>,
}

impl MinionQuery {
    pub fn new(q: &str) -> Result<Arc<Mutex<Self>>, SysinspectError> {
        let q = q.trim().trim_matches('/');
        let mut instance = Self { ..Default::default() };
        instance.scheme = SCHEME_MODEL.to_string(); // XXX: Drop "model://" scheme entirely

        let precise = q.contains('/');
        let sq: Vec<&str> = q.split(if precise { '/' } else { ':' }).filter(|s| !s.is_empty()).collect();
        match sq.len() {
            0 => {
                return Err(SysinspectError::ProtoError("No model has been targeted".to_string()));
            }
            1 => instance.src = sq[0].to_string(),
            2 => {
                instance.src = sq[0].to_string();
                if precise {
                    instance.entity = Some(sq[1].to_string());
                } else {
                    instance.labels = Some(sq[1].to_string());
                }
            }
            3 => {
                instance.src = sq[0].to_string();
                if precise {
                    instance.entity = Some(sq[1].to_string());
                    instance.state = Some(sq[2].to_string());
                }
            }
            _ => {}
        }

        Ok(Arc::new(Mutex::new(instance)))
    }

    /// Get target model name
    pub fn target(&self) -> &str {
        &self.src
    }

    /// Get entities, comma-separated
    pub fn entities(&self) -> Vec<String> {
        if let Some(entity) = &self.entity {
            return entity.split(',').map(|s| s.to_string()).collect();
        }

        vec![]
    }

    /// Get checkbook labels, comma-separated
    pub fn checkbook_labels(&self) -> Vec<String> {
        if let Some(l) = &self.labels {
            return l.split(',').map(|s| s.to_string()).collect();
        }
        vec![]
    }

    /// Get desired state of the model
    pub fn state(&self) -> Option<String> {
        if let Some(state) = &self.state {
            return Some(state.to_owned());
        }

        None
    }
}
