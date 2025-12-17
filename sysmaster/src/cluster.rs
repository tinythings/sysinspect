// Cluster management for sysmaster

use globset::Glob;
use libsysinspect::{SysinspectError, cfg::mmconf::ClusteredMinion, proto::MasterMessage};
use serde_yaml::Value;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::Mutex;

use crate::registry::mreg::MinionRegistry;

#[derive(Debug, Clone, Default)]
/// Representation of a clustered node
pub struct ClusterNode {
    hostname: String,
    mid: String,
    traits: HashMap<String, Value>,
}

/// Cluster node representation
impl ClusterNode {
    /// Create a new cluster node
    pub fn new(hostname: &str, mid: &str, traits: HashMap<String, Value>) -> ClusterNode {
        ClusterNode { hostname: hostname.to_string(), mid: mid.to_string(), traits }
    }

    /// Match hostname with glob pattern
    fn match_hostname(&self, pattern: &str) -> bool {
        match Glob::new(pattern) {
            Ok(g) => g.compile_matcher().is_match(&self.hostname),
            Err(_) => false,
        }
    }

    /// Match traits
    fn matches_traits(&self, traits: &HashMap<String, Value>) -> bool {
        for (k, v) in traits.iter() {
            if let Some(tv) = self.traits.get(k) {
                if tv != v {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone, Default)]
pub struct VirtualMinion {
    id: String,
    hostnames: Vec<String>,
    traits: HashMap<String, Value>,
    minions: Vec<ClusterNode>, // Configured physical minions
}

#[derive(Debug, Clone)]
pub struct VirtualMinionsCluster {
    mreg: Arc<Mutex<MinionRegistry>>,
    virtual_minions: Vec<VirtualMinion>, // Configured clustered minions
                                         /*
                                         Here must be a sort of state of the cluster, e.g., which minion is
                                         online, which is offline, last heartbeat time, their current load etc.
                                          */
}

impl VirtualMinionsCluster {
    pub fn new(cfg: Vec<ClusteredMinion>, mreg: Arc<Mutex<MinionRegistry>>) -> VirtualMinionsCluster {
        if cfg.is_empty() {
            return VirtualMinionsCluster { virtual_minions: Vec::new(), mreg };
        }
        let mut v_minions: Vec<VirtualMinion> = Vec::new();

        // Call all that stuff in self.init() later to keep mreg async
        for m in cfg.iter() {
            let nodes: Vec<ClusterNode> = Vec::new();
            let id: String = m.id().and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let traits: HashMap<String, Value> = match m.traits() {
                Some(t) => t.clone().into_iter().collect(),
                None => HashMap::new(),
            };

            for node in m.nodes().iter() {
                let _node_traits: HashMap<String, Value> = match node.traits() {
                    Some(t) => t.clone().into_iter().collect(),
                    None => HashMap::new(),
                };
                // TODO: find here minion nodes first
                //       First, glob the database, get minions matching the hostname pattern and/or traits.
                //       Then, create ClusterNode instances for each matched minion.

                //let node = ClusterNode::new(&node.hostname().cloned().unwrap_or_default(), &node.id().cloned().unwrap_or_default(), node_traits);
                //nodes.push(node);
            }

            let v_minion = VirtualMinion { id, hostnames: vec![m.hostname().cloned().unwrap_or_default()], traits, minions: nodes };
            v_minions.push(v_minion);
        }

        VirtualMinionsCluster { virtual_minions: v_minions, mreg }
    }

    pub async fn init(&self) -> Result<(), SysinspectError> {
        let minions = self.mreg.lock().await.get_by_hostname_or_ip("*")?;
        for mr in minions.iter() {
            log::info!("Discovered minion in cluster init: {:?}", mr.id());
        }
        Ok(())
    }

    /// Select a clustered minion by its hostname
    /// Returns a list of precise minion IDs matching the criteria.
    pub fn select(&self, _hostnames: Vec<String>, _traits: Option<HashMap<String, Value>>) -> Vec<String> {
        let ids: HashSet<String> = HashSet::new();

        ids.into_iter().collect()
    }

    /// Create MasterMessages to be sent to selected minions
    pub fn to_master_messages(&self, _query: &str) -> Vec<MasterMessage> {
        Vec::new()
    }

    /// Update cluster state by pong response from a minion
    pub fn update_state(&mut self) {}
}
