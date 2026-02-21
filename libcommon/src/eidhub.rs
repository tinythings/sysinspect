use colored::Colorize;
use std::{
    collections::HashMap,
    sync::OnceLock,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
struct EidItem {
    expires_at: Instant,
}

static EID_HUB: OnceLock<EidHub> = OnceLock::new();

/// Get the global EidHub instance, initializing it if necessary.
pub fn get_eidhub() -> &'static EidHub {
    EID_HUB.get_or_init(|| EidHub::new(Duration::from_secs(5)))
}

#[derive(Debug)]
pub struct EidHub {
    default_ttl: Duration,
    store: Mutex<HashMap<String, EidItem>>,
}

impl EidHub {
    pub fn new(default_ttl: Duration) -> Self {
        Self { default_ttl, store: Mutex::new(HashMap::new()) }
    }

    pub async fn add(&self, caller_id: &str, eid: &str) -> bool {
        if eid.contains('$') || eid.contains('*') {
            log::error!(
                "'{}' is registering a masked EID '{}' for action chain. Masked EIDs are not allowed.",
                caller_id.bright_yellow(),
                eid.bright_yellow()
            );
            return false;
        }

        let now = Instant::now();
        let mut m = self.store.lock().await;

        match m.get(eid) {
            Some(item) if item.expires_at > now => {
                return false; // still active
            }
            Some(_) => {
                // expired -> remove and continue to insert
                m.remove(eid);
            }
            None => {}
        }

        m.insert(eid.to_string(), EidItem { expires_at: now + self.default_ttl });

        true
    }

    pub async fn drop(&self, caller_id: &str, eid: &str) {
        // Ban wildcards / masks
        if eid.contains('$') || eid.contains('*') {
            log::error!("'{}' is attempting to drop a masked EID '{}'. Masked EIDs are not allowed.", caller_id.bright_yellow(), eid.bright_yellow(),);
            return;
        }

        let mut m = self.store.lock().await;
        m.remove(eid);
    }

    pub async fn get(&self, eid: &str) -> bool {
        let now = Instant::now();
        let mut m = self.store.lock().await;

        match m.get_mut(eid) {
            Some(item) if item.expires_at > now => {
                // auto-touch
                item.expires_at = now + self.default_ttl;
                true
            }
            Some(_) => {
                m.remove(eid);
                false
            }
            None => false,
        }
    }
}
