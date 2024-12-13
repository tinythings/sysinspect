/*
Session keeper.
Keeps connected minions and updates their uptime via heartbeat.
This prevents simultaenous connection of multiple minions on the same machine.
 */

use std::{collections::HashMap, time::Instant};

#[derive(Debug, Clone)]
struct Session {
    uptime: Instant,
    last: Instant,
    sid: String,
}

impl Session {
    pub fn new(sid: &str) -> Session {
        Session { last: Instant::now(), uptime: Instant::now(), sid: sid.to_string() }
    }

    pub fn age_sec(&self) -> u64 {
        self.last.elapsed().as_secs()
    }

    pub fn update(&mut self) {
        self.last = Instant::now()
    }

    pub fn uptime_sec(&self) -> u64 {
        self.uptime.elapsed().as_secs()
    }

    pub fn session_id(&self) -> String {
        self.sid.to_string()
    }
}

#[derive(Debug, Default, Clone)]
pub struct SessionKeeper {
    sessions: HashMap<String, Session>,
    lifetime: u64,
}

impl SessionKeeper {
    pub fn new(lifetime: u64) -> SessionKeeper {
        SessionKeeper { lifetime, ..Default::default() }
    }

    /// Collect the garbage (outdated sessions)
    #[allow(clippy::useless_conversion)] // Not useless: it has to be a copy because it self-shooting itself
    fn gc(&mut self) {
        for s in self.sessions.keys().into_iter().map(|s| s.to_string()).collect::<Vec<String>>() {
            self.alive(&s);
        }
    }

    /// Create a new session or update the existing
    pub fn ping(&mut self, mid: &str, sid: &str) {
        self.sessions.entry(mid.to_string()).or_insert_with(|| Session::new(sid)).update();
        self.gc();
    }

    /// Return uptime for a minion (seconds)
    pub fn uptime(&self, mid: &str) -> Option<u64> {
        self.sessions.get(mid).map(|s| s.uptime_sec())
    }

    /// Returns true if a minion is alive.
    pub fn alive(&mut self, mid: &str) -> bool {
        if let Some(session) = self.sessions.get(mid) {
            if session.age_sec() < self.lifetime {
                return true;
            }

            self.sessions.remove(mid);
        }
        false
    }

    pub(crate) fn exists(&mut self, mid: &str) -> bool {
        self.gc();
        self.sessions.contains_key(mid)
    }

    /// Get session Id for the minion
    pub(crate) fn get_id(&self, mid: &str) -> Option<String> {
        self.sessions.get(mid).map(|s| s.session_id())
    }

    pub(crate) fn remove(&mut self, id: &str) {
        self.sessions.remove(id);
    }
}
