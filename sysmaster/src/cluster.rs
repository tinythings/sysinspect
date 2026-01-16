// Cluster management for sysmaster
use crate::registry::{mreg::MinionRegistry, session::SessionKeeper, taskreg::TaskRegistry};
use colored::Colorize;
use globset::Glob;
use libsysinspect::{SysinspectError, cfg::mmconf::ClusteredMinion, proto::MasterMessage};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    vec,
};
use tokio::sync::Mutex;

const DEFAULT_TASK_JITTER: usize = 3; // Task tolerance

#[derive(Debug, Clone, Default)]
/// Representation of a clustered node
pub struct ClusterNode {
    mid: String,
    traits: HashMap<String, Value>,
    load_average: f32,
    io_bps: f64,    // disk I/O in bytes per second on writes
    cpu_usage: f32, // CPU usage percentage (overall)
}

/// Cluster node representation
impl ClusterNode {
    /// Create a new cluster node
    pub fn new(mid: &str, traits: HashMap<String, Value>) -> ClusterNode {
        ClusterNode { mid: mid.to_string(), traits, load_average: 0.0, io_bps: 0.0, cpu_usage: 0.0 }
    }

    /// Match hostname with glob pattern
    /// It takes data from the traits map, trying to get first short name, then FQDN.
    fn match_hostname(&self, pattern: &str) -> bool {
        for h in ["system.hostname", "system.hostname.fqdn"].iter() {
            if let Some(Value::String(hn)) = self.traits.get(*h) {
                return match Glob::new(pattern) {
                    Ok(g) => g.compile_matcher().is_match(hn),
                    Err(_) => false,
                };
            }
        }
        false
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

    /// Update load average
    pub fn set_load_average(&mut self, la: f32) {
        self.load_average = la;
    }

    /// Update I/O bps
    pub fn set_io_bps(&mut self, bps: f64) {
        self.io_bps = bps;
    }

    /// Update CPU usage
    pub fn set_cpu_usage(&mut self, cpu: f32) {
        self.cpu_usage = cpu;
    }
}

#[derive(Debug, Clone, Default)]
pub struct VirtualMinion {
    id: String,
    hostnames: Vec<String>,
    traits: HashMap<String, Value>,
    minions: Vec<ClusterNode>, // Configured physical minions
}
impl VirtualMinion {
    /// Match hostname with glob pattern
    /// It checks all configured hostnames for the virtual minion.
    fn match_hostname(&self, query: &str) -> bool {
        for hn in self.hostnames.iter() {
            match Glob::new(query) {
                Ok(g) => {
                    if g.compile_matcher().is_match(hn) {
                        return true;
                    }
                }
                Err(_) => continue,
            }
        }
        false
    }
}

#[derive(Debug, Clone)]
pub struct VirtualMinionsCluster {
    mreg: Arc<Mutex<MinionRegistry>>,
    session: Arc<Mutex<SessionKeeper>>,
    cfg: Vec<ClusteredMinion>,
    task_tracker: Arc<Mutex<TaskRegistry>>,
    virtual_minions: Vec<VirtualMinion>, // Configured clustered minions
    task_tolerance: usize,
}

impl VirtualMinionsCluster {
    pub fn new(
        cfg: Vec<ClusteredMinion>, mreg: Arc<Mutex<MinionRegistry>>, session: Arc<Mutex<SessionKeeper>>, task_tracker: Arc<Mutex<TaskRegistry>>,
    ) -> VirtualMinionsCluster {
        VirtualMinionsCluster { virtual_minions: Vec::new(), mreg, cfg, session, task_tracker, task_tolerance: DEFAULT_TASK_JITTER }
    }

    fn filter_traits(&self, traits: &HashMap<String, Value>) -> HashMap<String, Value> {
        let mut filtered = HashMap::new();
        let tk = ["system.hostname", "system.hostname.fqdn", "system.hostname.ip"];
        for (k, v) in traits.iter() {
            if tk.contains(&k.as_str()) {
                filtered.insert(k.clone(), v.clone());
            }
        }
        filtered
    }

    fn normalise_weights_percent(values: &[(String, f64)]) -> HashMap<String, f64> {
        let total: f64 = values.iter().map(|(_, v)| *v).sum();
        values.iter().map(|(id, v)| (id.clone(), 100.0 * (*v / total.max(1e-6)))).collect()
    }

    pub async fn init(&mut self) -> Result<(), SysinspectError> {
        let mut mreg = self.mreg.lock().await;

        // Call all that stuff in self.init() later to keep mreg async
        for m in self.cfg.iter() {
            log::debug!("Processing clustered minion: {:#?}", m);
            let mut nodes: Vec<ClusterNode> = Vec::new();
            let cm_traits: HashMap<String, Value> = match m.traits() {
                Some(t) => t.clone().into_iter().map(|(k, v)| (k, serde_json::to_value(v).unwrap_or(Value::Null))).collect(),
                None => HashMap::new(),
            };

            for node_scope in m.nodes().iter() {
                // Selectors
                let _id_sel = node_scope.id().and_then(|v| v.as_str()).unwrap_or_default();
                let hn_sel = node_scope.hostname().map(|s| s.as_str()).unwrap_or_default();
                let _qr_sel = node_scope.query().unwrap_or(&"".to_string());
                let _tr_sel: HashMap<String, Value> = match node_scope.traits() {
                    Some(t) => t.clone().into_iter().map(|(k, v)| (k, serde_json::to_value(v).unwrap_or(Value::Null))).collect(),
                    None => HashMap::new(),
                };

                /*
                Only hostname selector is implemented.
                To add more:
                1. nodes must be a hashset to avoid duplicates
                2. implement id_sel, qr_sel, tr_sel handling here
                 */

                // Get minion records matching the hostname or IP pattern
                if !hn_sel.is_empty() {
                    for mm in mreg.get_by_hostname_or_ip(hn_sel)?.iter() {
                        log::debug!("  Matched minion for clustered node {}: {:?}", hn_sel, mm.id());
                        nodes.push(ClusterNode::new(mm.id(), self.filter_traits(&mm.get_traits().clone())));
                    }
                }
            }

            if !nodes.is_empty() {
                self.virtual_minions.push(VirtualMinion {
                    id: m.id(),
                    hostnames: vec![m.hostname().cloned().unwrap_or_default()],
                    traits: cm_traits,
                    minions: nodes,
                });
            }
        }

        if self.virtual_minions.is_empty() {
            log::warn!("No clustered minions configured or found in the cluster.");
        } else {
            log::info!("Clustered minion details: {:#?}", self.virtual_minions);
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

    /// Get all minion IDs matching the query
    fn query_mids(&self, query: &str) -> Vec<String> {
        let mut mids: Vec<String> = Vec::new();
        log::debug!("Getting hostnames for virtual minions cluster with query: {query}");

        // Get all of them, if "*" is given
        if query == "*" {
            for vm in self.virtual_minions.iter() {
                for m in vm.minions.iter() {
                    mids.push(m.mid.clone());
                }
            }
        } else {
            for vm in self.virtual_minions.iter() {
                if vm.match_hostname(query) {
                    log::debug!("Virtual minion {} matched hostname query {}", vm.id, query);
                    for physical_minion in vm.minions.iter() {
                        mids.push(physical_minion.mid.clone());
                    }
                }
            }
        }
        mids
    }

    /// Decide the best-fit minion for a task based on current load and I/O pressure
    pub async fn decide(&self, query: &str) -> Option<String> {
        let mids = self.query_mids(query);
        if mids.is_empty() {
            return None;
        }

        // find alive minions
        let mut alive: Vec<String> = Vec::new();
        {
            let mut session = self.session.lock().await;
            for mid in mids.iter() {
                if session.alive(mid) {
                    alive.push(mid.clone());
                }
            }
        }

        if alive.is_empty() {
            return None;
        }

        // get IO rates
        let mut rates: Vec<(String, f64)> = Vec::with_capacity(alive.len());
        for mid in alive.iter() {
            let mut bps = 0.0;
            'outer: for vm in self.virtual_minions.iter() {
                for m in vm.minions.iter() {
                    if m.mid == *mid {
                        bps = m.io_bps;
                        break 'outer;
                    }
                }
            }
            rates.push((mid.clone(), bps.max(0.0)));
        }

        let weights: HashMap<String, f64> = Self::normalise_weights_percent(&rates);

        // minimum task count and lowest disk weight
        let tracker = self.task_tracker.lock().await;

        let mut min_tasks = usize::MAX;
        for mid in alive.iter() {
            min_tasks = min_tasks.min(tracker.minion_tasks(mid).len());
        }
        let cutoff = min_tasks.saturating_add(self.task_tolerance);

        let mut best_mid: Option<String> = None;
        let mut best_weight: f64 = f64::MAX;
        let mut best_tasks: usize = usize::MAX;

        for mid in alive.iter() {
            let tasks = tracker.minion_tasks(mid).len();
            if tasks > cutoff {
                continue;
            }

            let w = *weights.get(mid).unwrap_or(&0.0);

            // lower disk weight, then fewer tasks if weights tie
            if w < best_weight || (w == best_weight && tasks < best_tasks) {
                best_weight = w;
                best_tasks = tasks;
                best_mid = Some(mid.clone());
            }
        }

        let fmid = best_mid?;
        let mrec = match self.mreg.lock().await.get(&fmid) {
            Ok(r) => r,
            Err(err) => {
                log::error!("Unable to get minion record for {fmid}: {err}");
                return None;
            }
        };

        mrec.and_then(|rec| rec.get_traits().get("system.hostname.fqdn").and_then(|v| v.as_str()).map(|s| s.to_string()))
    }

    /// Call a query on the clustered minion.
    /// This will do the following:
    /// 1. Drop offline minions, if any (based on session state)
    /// 2. Analyse each minion status/load
    /// 3. Select the best-fit minion(s) to run the query
    ///
    /// This method must be called before master pings the minions with discovery
    /// type ping (not general) and thus updates the session state with alive/dead minions
    /// and their job load.
    pub async fn get_hostnames(&self, query: &str) -> Vec<String> {
        let mut hostnames: Vec<String> = Vec::new();

        // Construct hostnames from the query list
        for mid in self.query_mids(query).iter() {
            let mrec = match self.mreg.lock().await.get(mid) {
                Ok(mrec) => mrec,
                Err(err) => {
                    log::error!("Unable to get minion record for {mid}: {err}");
                    continue;
                }
            };
            if mrec.is_none() {
                log::error!("Minion record for {mid} not found");
                continue;
            }
            let mrec = mrec.unwrap();
            if self.session.lock().await.alive(mrec.id()) {
                let hn = mrec.get_traits().get("system.hostname.fqdn").and_then(|v| v.as_str()).unwrap_or_default();
                if !hn.is_empty() {
                    hostnames.push(hn.to_string());
                }
            } else {
                log::info!("Minion {} is offline, skipping", mid);
            }
        }

        hostnames
    }

    /// Update a physical minion stats, no matter where it belongs to
    pub fn update_stats(&mut self, mid: &str, load_average: f32, io_bps: f64) {
        for vm in self.virtual_minions.iter_mut() {
            for m in vm.minions.iter_mut() {
                if m.mid == mid {
                    log::info!("Updating load average for minion {}: {}, I/O bps: {}", mid, load_average, io_bps);
                    m.set_load_average(load_average);
                    m.set_io_bps(io_bps);
                    return;
                }
            }
        }
    }
}
